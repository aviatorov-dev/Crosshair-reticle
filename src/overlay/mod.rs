//! The platform thread: owns the overlay window, the tray icon, the global hotkeys and the Win32
//! message loop. The GUI talks to it through a `Controller`.

mod window;

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tiny_skia::{Paint, Pixmap, PixmapPaint, Rect, Transform};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_NOREPEAT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, KillTimer, PostMessageW, SetTimer, TranslateMessage, MSG,
    WM_HOTKEY, WM_TIMER,
};

use crate::config::{Crosshair, MonitorChoice, Shape};
use crate::core::{lock, pick_monitor, MonitorInfo, Shared, ToastKind};
use crate::hotkeys;
use crate::platform;
use crate::render;
use crate::tray::{TrayAction, TrayState, TrayUi};
use window::{OverlayWindow, WM_APP_DISPLAY, WM_APP_WAKE};

const TIMER_ID: usize = 1;
const TICK_MS: u32 = 50;
const TOPMOST_EVERY: Duration = Duration::from_secs(3);
const FLASH_FOR: Duration = Duration::from_secs(3);

#[derive(Debug)]
pub enum Command {
    /// Re-read the shared config and bring window, hotkeys and tray in line with it.
    Refresh,
    /// Temporarily unregister every hotkey (while the GUI records a new combination).
    SuspendHotkeys(bool),
    /// Show a frame around the selected monitor for a few seconds.
    FlashFrame,
}

/// Handle used by the GUI to talk to the platform thread.
pub struct Controller {
    tx: Sender<Command>,
    hwnd: AtomicIsize,
}

impl Controller {
    pub fn new() -> (Arc<Controller>, Receiver<Command>) {
        let (tx, rx) = channel();
        (
            Arc::new(Controller {
                tx,
                hwnd: AtomicIsize::new(0),
            }),
            rx,
        )
    }

    pub fn send(&self, cmd: Command) {
        if self.tx.send(cmd).is_ok() {
            let h = self.hwnd.load(Ordering::Acquire);
            if h != 0 {
                unsafe {
                    let _ = PostMessageW(HWND(h as *mut _), WM_APP_WAKE, WPARAM(0), LPARAM(0));
                }
            }
        }
    }

    pub fn refresh(&self) {
        self.send(Command::Refresh);
    }
}

/// Runs the platform thread. If it panics it is restarted (NFR-07); the overlay window and tray
/// icon are recreated from the shared state.
pub fn spawn(
    core: Shared,
    ctl: Arc<Controller>,
    rx: Receiver<Command>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("reticle-platform".into())
        .spawn(move || {
            let mut crashes = 0;
            loop {
                let res = catch_unwind(AssertUnwindSafe(|| run(&core, &ctl, &rx)));
                match res {
                    Ok(Ok(())) => break,
                    Ok(Err(e)) => tracing::error!("platform thread failed: {e:#}"),
                    Err(_) => tracing::error!("platform thread panicked, restarting"),
                }
                crashes += 1;
                if crashes > 5 {
                    tracing::error!("platform thread keeps failing, giving up");
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        })
        .expect("spawn platform thread")
}

#[derive(Clone, PartialEq)]
struct RenderKey {
    crosshair: Crosshair,
    dpi: u32,
    monitor: (i32, i32, i32, i32),
    flash: bool,
}

struct Platform<'a> {
    core: &'a Shared,
    window: OverlayWindow,
    tray: Option<TrayUi>,
    monitors: Vec<MonitorInfo>,
    registered: Vec<i32>,
    id_to_action: HashMap<i32, String>,
    wanted_hotkeys: Vec<(String, String)>,
    suspended: bool,
    last_key: Option<RenderKey>,
    shown: bool,
    flash_until: Option<Instant>,
    right_button_down: bool,
    last_raise: Instant,
    warned_missing_monitor: bool,
}

fn run(core: &Shared, ctl: &Controller, rx: &Receiver<Command>) -> anyhow::Result<()> {
    let window = OverlayWindow::create()?;
    ctl.hwnd.store(window.hwnd.0 as isize, Ordering::Release);

    let tray = match TrayUi::new() {
        Ok(t) => Some(t),
        Err(e) => {
            tracing::error!("tray icon unavailable: {e:#}");
            None
        }
    };

    let mut p = Platform {
        core,
        window,
        tray,
        monitors: Vec::new(),
        registered: Vec::new(),
        id_to_action: HashMap::new(),
        wanted_hotkeys: Vec::new(),
        suspended: false,
        last_key: None,
        shown: false,
        flash_until: None,
        right_button_down: false,
        last_raise: Instant::now(),
        warned_missing_monitor: false,
    };
    unsafe {
        SetTimer(p.window.hwnd, TIMER_ID, TICK_MS, None);
    }
    p.drain_commands(rx);
    p.refresh();

    let mut msg = MSG::default();
    loop {
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if r.0 <= 0 {
            break;
        }
        match msg.message {
            WM_HOTKEY => p.on_hotkey(msg.wParam.0 as i32),
            WM_TIMER if msg.wParam.0 == TIMER_ID => p.on_tick(),
            WM_APP_WAKE => {
                p.drain_commands(rx);
                p.refresh();
            }
            WM_APP_DISPLAY => {
                p.last_key = None;
                p.refresh();
            }
            _ => unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }
        p.poll_tray();
    }
    Ok(())
}

impl<'a> Platform<'a> {
    fn drain_commands(&mut self, rx: &Receiver<Command>) {
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                Command::Refresh => {}
                Command::SuspendHotkeys(s) => {
                    self.suspended = s;
                    self.sync_hotkeys(&[]);
                }
                Command::FlashFrame => {
                    self.flash_until = Some(Instant::now() + FLASH_FOR);
                    self.last_key = None;
                }
            }
        }
    }

    // ------------------------------------------------------------ refresh

    fn refresh(&mut self) {
        self.monitors = platform::enum_monitors();

        // snapshot everything we need, then release the lock before touching Win32
        let (crosshair, choice, dpi_on, visible, bindings, bitmap, tray_state) = {
            let mut guard = lock(self.core);
            let core = &mut *guard;
            core.monitors = self.monitors.clone();
            let preset = core.config.active().clone();
            let paths = core.paths.clone();
            let bitmap = preset
                .crosshair
                .image
                .as_ref()
                .filter(|_| preset.crosshair.shape == Shape::Image)
                .and_then(|i| core.images.get(&paths, &i.file));
            let tray_state = TrayState {
                visible: core.visible,
                lang: core.config.general.language,
                active_id: preset.id.clone(),
                active_name: preset.name.clone(),
                presets: core
                    .config
                    .presets
                    .iter()
                    .map(|p| (p.id.clone(), p.name.clone()))
                    .collect(),
            };
            (
                preset.crosshair,
                core.config.general.monitor.clone(),
                core.config.general.scale_with_dpi,
                core.visible
                    && core.config.general.show_crosshair
                    && !(core.config.general.hide_crosshair_on_rmb && self.right_button_down),
                core.config.hotkey_bindings(),
                bitmap,
                tray_state,
            )
        };

        self.draw(&crosshair, &choice, dpi_on, visible, bitmap.as_deref());
        self.sync_hotkeys(&bindings);
        if let Some(t) = &mut self.tray {
            t.update(&tray_state);
        }
        self.repaint_gui();
    }

    fn draw(
        &mut self,
        crosshair: &Crosshair,
        choice: &MonitorChoice,
        dpi_on: bool,
        visible: bool,
        bitmap: Option<&image::RgbaImage>,
    ) {
        let Some(mon) = pick_monitor(&self.monitors, choice).cloned() else {
            return;
        };
        if let MonitorChoice::Named(_) = choice {
            let missing = !self
                .monitors
                .iter()
                .any(|m| matches!(choice, MonitorChoice::Named(n) if *n == m.name));
            if missing && !self.warned_missing_monitor {
                self.warned_missing_monitor = true;
                lock(self.core).toast(
                    ToastKind::Warning,
                    "Выбранный монитор отключён, прицел показан на основном / Selected monitor is gone, using the primary one",
                );
            } else if !missing {
                self.warned_missing_monitor = false;
            }
        }

        let flash = self.flash_until.is_some();
        let dpi = if dpi_on { mon.dpi_scale() } else { 1.0 };
        let key = RenderKey {
            crosshair: crosshair.clone(),
            dpi: dpi.to_bits(),
            monitor: (mon.left, mon.top, mon.width, mon.height),
            flash,
        };
        if self.last_key.as_ref() != Some(&key) {
            let pm = render::render(crosshair, dpi, bitmap);
            let (cx, cy) = (mon.left + mon.width / 2, mon.top + mon.height / 2);
            let result = if flash {
                let frame = flash_image(&mon, &pm, crosshair.offset);
                self.window.present(&frame, mon.left, mon.top)
            } else {
                let x = cx - pm.width() as i32 / 2 + crosshair.offset.0;
                let y = cy - pm.height() as i32 / 2 + crosshair.offset.1;
                self.window.present(&pm, x, y)
            };
            match result {
                Ok(()) => self.last_key = Some(key),
                Err(e) => tracing::error!("cannot update overlay: {e:#}"),
            }
        }

        let want_shown = visible || flash;
        if want_shown != self.shown {
            if want_shown {
                self.window.show();
                self.window.raise();
            } else {
                self.window.hide();
            }
            self.shown = want_shown;
        }
    }

    // ------------------------------------------------------------ hotkeys

    /// `bindings` empty = reuse the last wanted set (used by suspend / resume).
    fn sync_hotkeys(&mut self, bindings: &[(String, String)]) {
        if !bindings.is_empty() {
            if bindings == self.wanted_hotkeys.as_slice()
                && !self.registered.is_empty()
                && !self.suspended
            {
                return;
            }
            self.wanted_hotkeys = bindings.to_vec();
        }
        for id in self.registered.drain(..) {
            unsafe {
                let _ = UnregisterHotKey(self.window.hwnd, id);
            }
        }
        self.id_to_action.clear();
        if self.suspended {
            return;
        }
        let mut errors: HashMap<String, String> = HashMap::new();
        for (i, (action, combo)) in self.wanted_hotkeys.iter().enumerate() {
            let id = i as i32 + 1;
            let Ok(hk) = hotkeys::parse(combo) else {
                errors.insert(action.clone(), format!("Invalid hotkey: {combo}"));
                continue;
            };
            let res = unsafe {
                RegisterHotKey(
                    self.window.hwnd,
                    id,
                    HOT_KEY_MODIFIERS(hk.mods) | MOD_NOREPEAT,
                    hk.vk,
                )
            };
            match res {
                Ok(()) => {
                    self.registered.push(id);
                    self.id_to_action.insert(id, action.clone());
                }
                Err(e) => {
                    tracing::warn!("RegisterHotKey({combo}) failed: {e}");
                    errors.insert(action.clone(), format!("{combo} is already in use"));
                }
            }
        }
        let changed = {
            let mut core = lock(self.core);
            let changed = core.hotkey_errors != errors;
            core.hotkey_errors = errors;
            changed
        };
        if changed {
            self.repaint_gui();
        }
    }

    fn on_hotkey(&mut self, id: i32) {
        let Some(action) = self.id_to_action.get(&id).cloned() else {
            return;
        };
        self.run_action(&action);
    }

    fn run_action(&mut self, action: &str) {
        let mut open_settings = false;
        {
            let mut core = lock(self.core);
            match action {
                "toggle" => core.toggle_visible(),
                "next_preset" => core.cycle_preset(1),
                "prev_preset" => core.cycle_preset(-1),
                "open_settings" => open_settings = true,
                other => {
                    if let Some(id) = other.strip_prefix("preset:") {
                        core.activate_preset(id);
                    }
                }
            }
        }
        if open_settings {
            platform::show_settings_window();
        }
        self.refresh();
    }

    // --------------------------------------------------------------- tray

    fn poll_tray(&mut self) {
        let Some(tray) = &self.tray else { return };
        let actions = tray.poll();
        for a in actions {
            match a {
                TrayAction::Toggle => self.run_action("toggle"),
                TrayAction::OpenSettings => {
                    platform::show_settings_window();
                }
                TrayAction::Preset(id) => self.run_action(&format!("preset:{id}")),
                TrayAction::Quit => self.quit(),
            }
        }
    }

    fn quit(&mut self) -> ! {
        lock(self.core).save_now();
        // drop the tray icon first, otherwise a ghost icon stays until the mouse hovers it
        self.tray = None;
        unsafe {
            let _ = KillTimer(self.window.hwnd, TIMER_ID);
        }
        std::process::exit(0);
    }

    // --------------------------------------------------------------- tick

    fn on_tick(&mut self) {
        lock(self.core).save_if_due();
        let (enabled, hide_on_rmb, manually_visible) = {
            let core = lock(self.core);
            (
                core.config.general.show_crosshair,
                core.config.general.hide_crosshair_on_rmb,
                core.visible,
            )
        };
        let down = platform::right_button_down();
        let button_changed = down != self.right_button_down;
        self.right_button_down = down;
        if button_changed && enabled && hide_on_rmb && manually_visible {
            self.refresh();
        }
        if let Some(until) = self.flash_until {
            if Instant::now() >= until {
                self.flash_until = None;
                self.last_key = None;
                self.refresh();
            }
        }
        if self.shown && self.last_raise.elapsed() >= TOPMOST_EVERY {
            self.window.raise();
            self.last_raise = Instant::now();
        }
    }

    fn repaint_gui(&self) {
        lock(self.core).request_repaint();
    }
}

impl Drop for Platform<'_> {
    fn drop(&mut self) {
        for id in self.registered.drain(..) {
            unsafe {
                let _ = UnregisterHotKey(self.window.hwnd, id);
            }
        }
        unsafe {
            let _ = KillTimer(self.window.hwnd, TIMER_ID);
        }
    }
}

/// Monitor-sized image with a coloured frame, centre guides and the crosshair in the middle.
fn flash_image(mon: &MonitorInfo, crosshair: &Pixmap, offset: (i32, i32)) -> Pixmap {
    let w = mon.width.clamp(16, 8192) as u32;
    let h = mon.height.clamp(16, 8192) as u32;
    let mut pm = Pixmap::new(w, h).expect("monitor size is valid");
    let mut frame = Paint::default();
    frame.set_color_rgba8(76, 141, 255, 255);
    let mut guide = Paint::default();
    guide.set_color_rgba8(76, 141, 255, 110);
    let (wf, hf) = (w as f32, h as f32);
    let b = 4.0;
    for (l, t, r, bt) in [
        (0.0, 0.0, wf, b),
        (0.0, hf - b, wf, hf),
        (0.0, 0.0, b, hf),
        (wf - b, 0.0, wf, hf),
    ] {
        if let Some(rc) = Rect::from_ltrb(l, t, r, bt) {
            pm.fill_rect(rc, &frame, Transform::identity(), None);
        }
    }
    for (l, t, r, bt) in [
        (0.0, hf / 2.0 - 0.5, wf, hf / 2.0 + 0.5),
        (wf / 2.0 - 0.5, 0.0, wf / 2.0 + 0.5, hf),
    ] {
        if let Some(rc) = Rect::from_ltrb(l, t, r, bt) {
            pm.fill_rect(rc, &guide, Transform::identity(), None);
        }
    }
    let x = (w as i32 / 2) - crosshair.width() as i32 / 2 + offset.0;
    let y = (h as i32 / 2) - crosshair.height() as i32 / 2 + offset.1;
    pm.draw_pixmap(
        x,
        y,
        crosshair.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    pm
}
