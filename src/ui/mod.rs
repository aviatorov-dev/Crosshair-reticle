//! The egui settings window. Closing it only hides it: the overlay, hotkeys and tray live on the
//! platform thread (see `overlay`).

mod editor;
mod hotkeys_page;
mod onboarding;
mod presets;
mod settings;
mod theme;
mod widgets;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align2, Color32, FontId, Id, Pos2, Rect, RichText, Sense, Stroke, TextureHandle, Vec2,
    ViewportCommand,
};

use crate::brand::icon_rgba;
use crate::config::{Config, Crosshair, Lang, Preset, Rgba};
use crate::core::{lock, Core, Shared, ToastKind};
use crate::hotkeys;
use crate::overlay::{Command, Controller};
use crate::platform;
use theme::Palette;

pub fn tr(l: Lang, ru: &'static str, en: &'static str) -> &'static str {
    match l {
        Lang::Ru => ru,
        Lang::En => en,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Editor,
    Presets,
    Hotkeys,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PreviewBg {
    Game,
    Light,
    Dark,
    Checker,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ColorSlot {
    Main,
    Outline,
    Tint,
}

pub struct Eye {
    pub slot: ColorSlot,
    /// The click that opened the eyedropper must be released before a new click counts.
    pub seen_up: bool,
}

pub struct Undo {
    pub preset: Preset,
    pub index: usize,
    pub at: Instant,
}

pub struct UiToast {
    pub kind: ToastKind,
    pub text: String,
    pub created: Instant,
    pub undo: bool,
}

/// Work that needs the `Core` lock to be released (file dialogs, clipboard).
pub enum Action {
    ImportFile,
    PasteClipboard,
    ExportPreset(String),
    CopyPreset(String),
    ExportAll,
    PickPng,
}

pub struct UiState {
    pub page: Page,
    pub preview_tex: Option<TextureHandle>,
    pub preview_key: Option<(Crosshair, u32)>,
    pub preview_bg: PreviewBg,
    pub preview_zoom: f32,
    pub thumbs: HashMap<String, (Crosshair, TextureHandle)>,
    pub toasts: Vec<UiToast>,
    pub undo: Option<Undo>,
    /// Hotkey action currently being recorded.
    pub recording: Option<String>,
    pub rename: Option<(String, String)>,
    pub eye: Option<Eye>,
    pub search: String,
    pub onboarding: Option<u8>,
    pub confirm_reset: bool,
    applied_theme: Option<(bool, bool)>,
}

pub struct Env<'a> {
    pub core: &'a mut Core,
    pub st: &'a mut UiState,
    pub ctl: &'a Controller,
    pub actions: &'a mut Vec<Action>,
    pub pal: Palette,
    pub lang: Lang,
    /// Set when the config changed: it is then saved and the overlay refreshed.
    pub changed: bool,
}

impl Env<'_> {
    pub fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.st.toasts.push(UiToast {
            kind,
            text: text.into(),
            created: Instant::now(),
            undo: false,
        });
        self.st.toasts.truncate(5);
    }
}

pub struct ReticleApp {
    core: Shared,
    ctl: Arc<Controller>,
    st: UiState,
}

impl ReticleApp {
    pub fn new(cc: &eframe::CreationContext<'_>, core: Shared, ctl: Arc<Controller>) -> Self {
        let ctx = cc.egui_ctx.clone();
        let onboarding = {
            let mut c = lock(&core);
            c.repaint = Some(Box::new(move || ctx.request_repaint()));
            if c.config.general.onboarded {
                None
            } else {
                Some(0)
            }
        };
        ReticleApp {
            core,
            ctl,
            st: UiState {
                page: Page::Editor,
                preview_tex: None,
                preview_key: None,
                preview_bg: PreviewBg::Game,
                preview_zoom: 2.0,
                thumbs: HashMap::new(),
                toasts: Vec::new(),
                undo: None,
                recording: None,
                rename: None,
                eye: None,
                search: String::new(),
                onboarding,
                confirm_reset: false,
                applied_theme: None,
            },
        }
    }

    fn handle_recording(&mut self, ctx: &egui::Context) {
        let Some(action) = self.st.recording.clone() else {
            return;
        };
        let focused = ctx.input(|i| i.viewport().focused).unwrap_or(true);
        let mut finish = !focused;
        let mut assign: Option<String> = None;
        for ev in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = ev
            {
                if key == egui::Key::Escape {
                    finish = true;
                } else if let Some(s) = hotkeys::from_parts(
                    modifiers.ctrl,
                    modifiers.alt,
                    modifiers.shift,
                    false,
                    &format!("{key:?}"),
                ) {
                    assign = Some(s);
                    finish = true;
                }
            }
        }
        if finish {
            self.st.recording = None;
            if let Some(combo) = assign {
                let mut core = lock(&self.core);
                core.config.set_hotkey(&action, Some(combo));
                core.touch();
            }
            self.ctl.send(Command::SuspendHotkeys(false));
            self.ctl.refresh();
        }
    }

    fn handle_eyedropper(&mut self, ctx: &egui::Context) {
        let Some(eye) = &mut self.st.eye else { return };
        ctx.request_repaint_after(Duration::from_millis(16));
        if platform::escape_down() {
            self.st.eye = None;
            return;
        }
        let down = platform::left_button_down();
        if !down {
            eye.seen_up = true;
        }
        if eye.seen_up && down {
            let slot = eye.slot;
            self.st.eye = None;
            if let Some([r, g, b]) = platform::cursor_pixel() {
                let mut core = lock(&self.core);
                let a = {
                    let c = &mut core.config.active_mut().crosshair;
                    match slot {
                        ColorSlot::Main => Some(&mut c.color),
                        ColorSlot::Outline => Some(&mut c.outline.color),
                        ColorSlot::Tint => c.image.as_mut().and_then(|i| i.tint.as_mut()),
                    }
                    .map(|col| {
                        col.0[0] = r;
                        col.0[1] = g;
                        col.0[2] = b;
                        *col
                    })
                };
                if let Some(c) = a {
                    core.config.push_recent_color(c);
                    core.touch();
                }
                drop(core);
                self.ctl.refresh();
            }
        }
    }

    fn run_actions(&mut self, actions: Vec<Action>) {
        for a in actions {
            match a {
                Action::PickPng => {
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter("PNG", &["png"])
                        .pick_file()
                    else {
                        continue;
                    };
                    let result = {
                        let mut core = lock(&self.core);
                        let r = core.import_png_file(&path);
                        if let Ok(name) = &r {
                            let c = &mut core.config.active_mut().crosshair;
                            let scale = c.image.as_ref().map(|i| i.scale).unwrap_or(1.0);
                            let tint = c.image.as_ref().and_then(|i| i.tint);
                            c.image = Some(crate::config::ImageSource {
                                file: name.clone(),
                                scale,
                                tint,
                            });
                            c.shape = crate::config::Shape::Image;
                            core.touch();
                        }
                        r
                    };
                    match result {
                        Ok(_) => self.ctl.refresh(),
                        Err(e) => self.toast(ToastKind::Error, format!("{e:#}")),
                    }
                }
                Action::ImportFile => {
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .pick_file()
                    else {
                        continue;
                    };
                    match std::fs::read_to_string(&path) {
                        Ok(text) => self.import_text(&text),
                        Err(e) => self.toast(ToastKind::Error, format!("{e}")),
                    }
                }
                Action::PasteClipboard => {
                    let text = arboard::Clipboard::new().and_then(|mut c| c.get_text());
                    match text {
                        Ok(t) => self.import_text(&t),
                        Err(_) => {
                            self.toast(ToastKind::Error, "Буфер обмена пуст / Clipboard is empty")
                        }
                    }
                }
                Action::CopyPreset(id) => {
                    let core = lock(&self.core);
                    let Some(p) = core.config.preset(&id).cloned() else {
                        continue;
                    };
                    let png = p
                        .crosshair
                        .image
                        .as_ref()
                        .filter(|_| p.crosshair.shape == crate::config::Shape::Image)
                        .and_then(|i| core.image_bytes(&i.file));
                    drop(core);
                    let s = crate::config::SharedPreset::new(&p, png).to_share_string();
                    match arboard::Clipboard::new().and_then(|mut c| c.set_text(s)) {
                        Ok(()) => self.toast(ToastKind::Success, "Скопировано / Copied"),
                        Err(e) => self.toast(ToastKind::Error, format!("{e}")),
                    }
                }
                Action::ExportPreset(id) => {
                    let core = lock(&self.core);
                    let Some(p) = core.config.preset(&id).cloned() else {
                        continue;
                    };
                    let png = p
                        .crosshair
                        .image
                        .as_ref()
                        .filter(|_| p.crosshair.shape == crate::config::Shape::Image)
                        .and_then(|i| core.image_bytes(&i.file));
                    drop(core);
                    let json = crate::config::SharedPreset::new(&p, png).to_json();
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .set_file_name(format!("{}.json", p.id))
                        .save_file()
                    else {
                        continue;
                    };
                    match std::fs::write(&path, json) {
                        Ok(()) => self.toast(ToastKind::Success, "Экспортировано / Exported"),
                        Err(e) => self.toast(ToastKind::Error, format!("{e}")),
                    }
                }
                Action::ExportAll => {
                    let core = lock(&self.core);
                    let list: Vec<_> = core
                        .config
                        .presets
                        .iter()
                        .filter(|p| !p.builtin)
                        .map(|p| {
                            let png = p
                                .crosshair
                                .image
                                .as_ref()
                                .filter(|_| p.crosshair.shape == crate::config::Shape::Image)
                                .and_then(|i| core.image_bytes(&i.file));
                            crate::config::SharedPreset::new(p, png)
                        })
                        .collect();
                    drop(core);
                    if list.is_empty() {
                        self.toast(ToastKind::Info, "Нет своих пресетов / No custom presets");
                        continue;
                    }
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .set_file_name("reticle-presets.json")
                        .save_file()
                    else {
                        continue;
                    };
                    let json = serde_json::to_string_pretty(&list).unwrap_or_default();
                    match std::fs::write(&path, json) {
                        Ok(()) => self.toast(ToastKind::Success, "Экспортировано / Exported"),
                        Err(e) => self.toast(ToastKind::Error, format!("{e}")),
                    }
                }
            }
        }
    }

    fn import_text(&mut self, text: &str) {
        match crate::config::SharedPreset::parse_many(text) {
            Ok(list) => {
                let n = list.len();
                let mut core = lock(&self.core);
                let mut last = None;
                for s in list {
                    let mut p = s.preset.clone();
                    if let (Some(bytes), Some(img)) = (s.png_bytes(), p.crosshair.image.as_mut()) {
                        if let Ok(name) = core.import_png_bytes(&bytes) {
                            img.file = name;
                        }
                    }
                    last = Some(core.config.add_preset(&p.name, p.crosshair));
                }
                if let Some(id) = last {
                    core.config.set_active(&id);
                }
                core.touch();
                drop(core);
                self.ctl.refresh();
                self.toast(ToastKind::Success, format!("Импортировано / Imported: {n}"));
            }
            Err(e) => self.toast(ToastKind::Error, format!("{e:#}")),
        }
    }

    fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.st.toasts.push(UiToast {
            kind,
            text: text.into(),
            created: Instant::now(),
            undo: false,
        });
        self.st.toasts.truncate(5);
    }
}

impl eframe::App for ReticleApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // closing only hides the window
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        }

        self.handle_recording(ctx);
        self.handle_eyedropper(ctx);

        let mut actions: Vec<Action> = Vec::new();
        let changed;
        {
            let mut guard = lock(&self.core);
            let core = &mut *guard;

            // theme / zoom
            let dark = theme::is_dark(ctx, core.config.general.theme);
            let reduce = core.config.general.reduce_motion;
            if self.st.applied_theme != Some((dark, reduce)) {
                theme::apply(ctx, &theme::palette(dark), reduce);
                self.st.applied_theme = Some((dark, reduce));
            }
            let zoom = core.config.general.ui_scale;
            if (ctx.zoom_factor() - zoom).abs() > 0.001 {
                ctx.set_zoom_factor(zoom);
            }
            let pal = theme::palette(dark);

            // toasts raised by the platform thread
            for t in core.toasts.drain(..) {
                if t.created.elapsed() < Duration::from_secs(5) {
                    self.st.toasts.push(UiToast {
                        kind: t.kind,
                        text: t.text,
                        created: t.created,
                        undo: false,
                    });
                }
            }

            // remember the window size
            if let Some(r) = ctx.input(|i| i.viewport().inner_rect) {
                let maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
                let minimized = ctx.input(|i| i.viewport().minimized).unwrap_or(false);
                let z = ctx.zoom_factor();
                let size = [r.width() * z, r.height() * z];
                if !maximized && !minimized && size[0] > 100.0 {
                    let old = core.config.general.window_size;
                    if old
                        .map(|o| (o[0] - size[0]).abs() > 1.0 || (o[1] - size[1]).abs() > 1.0)
                        .unwrap_or(true)
                    {
                        core.config.general.window_size = Some(size);
                        core.touch();
                    }
                }
            }

            let mut env = Env {
                lang: core.config.general.language,
                core,
                st: &mut self.st,
                ctl: &self.ctl,
                actions: &mut actions,
                pal,
                changed: false,
            };
            draw(ctx, &mut env);
            changed = env.changed;
        }

        if changed {
            lock(&self.core).touch();
            self.ctl.refresh();
        }
        self.st
            .toasts
            .retain(|t| t.created.elapsed() < Duration::from_secs(if t.undo { 6 } else { 4 }));
        if !self.st.toasts.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if !actions.is_empty() {
            self.run_actions(actions);
        }
    }
}

// ---------------------------------------------------------------------------- layout

fn draw(ctx: &egui::Context, env: &mut Env) {
    title_bar(ctx, env);
    status_bar(ctx, env);
    sidebar(ctx, env);
    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(env.pal.bg)
                .inner_margin(egui::Margin::same(16.0)),
        )
        .show(ctx, |ui| match env.st.page {
            Page::Editor => editor::show(ui, env),
            Page::Presets => presets::show(ui, env),
            Page::Hotkeys => hotkeys_page::show(ui, env),
            Page::Settings => settings::show(ui, env),
        });
    presets::dialogs(ctx, env);
    onboarding::show(ctx, env);
    eyedropper_hint(ctx, env);
    toasts(ctx, env);
}

fn title_bar(ctx: &egui::Context, env: &mut Env) {
    let pal = env.pal;
    egui::TopBottomPanel::top("title")
        .exact_height(36.0)
        .frame(egui::Frame::none().fill(pal.bg))
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            let drag = ui.interact(rect, Id::new("title_drag"), Sense::click_and_drag());
            if drag.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            let maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
            if drag.double_clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }

            // logo + name
            let logo = ui.painter();
            let c = Pos2::new(rect.left() + 22.0, rect.center().y);
            logo.circle_stroke(c, 7.0, Stroke::new(2.0_f32, pal.accent));
            logo.circle_filled(c, 1.8, pal.accent);
            logo.text(
                Pos2::new(rect.left() + 38.0, rect.center().y),
                Align2::LEFT_CENTER,
                "Crosshair reticle",
                FontId::proportional(15.0),
                pal.text,
            );

            // window buttons
            let bw = 42.0;
            let mut x = rect.right() - bw;
            for kind in [2u8, 1, 0] {
                let r = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(bw, rect.height()));
                let resp = ui.interact(r, Id::new(("title_btn", kind)), Sense::click());
                let hot = resp.hovered();
                if hot {
                    ui.painter()
                        .rect_filled(r, 0.0, if kind == 2 { pal.danger } else { pal.card });
                }
                let col = if hot && kind == 2 {
                    Color32::WHITE
                } else {
                    pal.text
                };
                let s = Stroke::new(1.4_f32, col);
                let m = r.center();
                let p = ui.painter();
                match kind {
                    0 => {
                        p.line_segment([m + Vec2::new(-5.0, 3.0), m + Vec2::new(5.0, 3.0)], s);
                    }
                    1 => {
                        p.rect_stroke(Rect::from_center_size(m, Vec2::splat(10.0)), 1.0, s);
                    }
                    _ => {
                        p.line_segment([m + Vec2::new(-5.0, -5.0), m + Vec2::new(5.0, 5.0)], s);
                        p.line_segment([m + Vec2::new(-5.0, 5.0), m + Vec2::new(5.0, -5.0)], s);
                    }
                }
                if resp.clicked() {
                    match kind {
                        0 => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
                        1 => ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized)),
                        _ => ctx.send_viewport_cmd(ViewportCommand::Visible(false)),
                    }
                }
                resp.on_hover_text(match kind {
                    0 => tr(env.lang, "Свернуть", "Minimize"),
                    1 => tr(env.lang, "Развернуть", "Maximize"),
                    _ => tr(env.lang, "Скрыть в трей", "Hide to tray"),
                });
                x -= bw;
            }
        });
}

fn sidebar(ctx: &egui::Context, env: &mut Env) {
    let pal = env.pal;
    let l = env.lang;
    egui::SidePanel::left("nav")
        .exact_width(160.0)
        .resizable(false)
        .frame(
            egui::Frame::none()
                .fill(pal.panel)
                .inner_margin(egui::Margin::same(10.0)),
        )
        .show(ctx, |ui| {
            ui.add_space(6.0);
            for (page, ru, en) in [
                (Page::Editor, "Прицел", "Crosshair"),
                (Page::Presets, "Пресеты", "Presets"),
                (Page::Hotkeys, "Горячие клавиши", "Hotkeys"),
                (Page::Settings, "Настройки", "Settings"),
            ] {
                if widgets::nav_item(ui, &pal, tr(l, ru, en), env.st.page == page) {
                    env.st.page = page;
                }
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let mut on = env.core.visible;
                    if widgets::switch(ui, &pal, &mut on).changed() {
                        env.core.visible = on;
                        env.changed = true;
                    }
                    ui.label(if on {
                        tr(l, "Оверлей вкл", "Overlay on")
                    } else {
                        tr(l, "Оверлей выкл", "Overlay off")
                    });
                });
            });
        });
}

fn status_bar(ctx: &egui::Context, env: &mut Env) {
    let pal = env.pal;
    let l = env.lang;
    egui::TopBottomPanel::bottom("status")
        .exact_height(28.0)
        .frame(
            egui::Frame::none()
                .fill(pal.panel)
                .inner_margin(egui::Margin::symmetric(12.0, 4.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (color, text) = if env.core.visible {
                    (pal.success, tr(l, "Оверлей активен", "Overlay active"))
                } else {
                    (pal.muted, tr(l, "Оверлей скрыт", "Overlay hidden"))
                };
                widgets::status_dot(ui, color, text, &pal);
                let mon =
                    crate::core::pick_monitor(&env.core.monitors, &env.core.config.general.monitor);
                if let Some(m) = mon {
                    let idx = env
                        .core
                        .monitors
                        .iter()
                        .position(|x| x.name == m.name)
                        .unwrap_or(0)
                        + 1;
                    widgets::muted(
                        ui,
                        &pal,
                        &format!(
                            "{} {} {}x{}",
                            tr(l, "Монитор", "Monitor"),
                            idx,
                            m.width,
                            m.height
                        ),
                    );
                }
                widgets::muted(
                    ui,
                    &pal,
                    &format!(
                        "{}: {}",
                        tr(l, "Пресет", "Preset"),
                        env.core.config.active().name
                    ),
                );
                if !env.core.hotkey_errors.is_empty() {
                    ui.label(
                        RichText::new(tr(
                            l,
                            "Часть горячих клавиш занята другим приложением",
                            "Some hotkeys are taken by another app",
                        ))
                        .small()
                        .color(pal.warn),
                    );
                }
            });
        });
}

fn eyedropper_hint(ctx: &egui::Context, env: &mut Env) {
    if env.st.eye.is_none() {
        return;
    }
    let pal = env.pal;
    let l = env.lang;
    let px = platform::cursor_pixel();
    egui::Area::new(Id::new("eye_hint"))
        .anchor(Align2::CENTER_TOP, [0.0, 44.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some([r, g, b]) = px {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                        ui.painter()
                            .rect_filled(rect, 4.0, Color32::from_rgb(r, g, b));
                        ui.label(RichText::new(Rgba([r, g, b, 255]).to_hex()).monospace());
                    }
                    ui.label(tr(
                        l,
                        "Кликните в любом месте экрана. Esc - отмена",
                        "Click anywhere on screen. Esc cancels",
                    ));
                });
            });
        });
    let _ = pal;
}

fn toasts(ctx: &egui::Context, env: &mut Env) {
    if env.st.toasts.is_empty() {
        return;
    }
    let pal = env.pal;
    let l = env.lang;
    let mut undo_clicked = false;
    egui::Area::new(Id::new("toasts"))
        .anchor(Align2::RIGHT_BOTTOM, [-16.0, -40.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            for t in env.st.toasts.iter().rev() {
                let (color, prefix) = match t.kind {
                    ToastKind::Info => (pal.accent, ""),
                    ToastKind::Success => (pal.success, ""),
                    ToastKind::Warning => (pal.warn, tr(l, "Внимание: ", "Warning: ")),
                    ToastKind::Error => (pal.danger, tr(l, "Ошибка: ", "Error: ")),
                };
                egui::Frame::none()
                    .fill(pal.card)
                    .stroke(Stroke::new(1.0_f32, color))
                    .rounding(8.0)
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                    .show(ui, |ui| {
                        ui.set_max_width(340.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(format!("{prefix}{}", t.text)).color(pal.text));
                            if t.undo && ui.link(tr(l, "Отменить", "Undo")).clicked() {
                                undo_clicked = true;
                            }
                        });
                    });
                ui.add_space(6.0);
            }
        });
    if undo_clicked {
        if let Some(u) = env.st.undo.take() {
            env.core.config.restore_preset(u.preset, u.index);
            env.st.toasts.retain(|t| !t.undo);
            env.changed = true;
        }
    }
}

/// Icon for the window / taskbar.
pub fn window_icon() -> egui::IconData {
    egui::IconData {
        rgba: icon_rgba(64, true),
        width: 64,
        height: 64,
    }
}

/// Starts the GUI; returns when the event loop ends.
pub fn run(core: Shared, ctl: Arc<Controller>) -> eframe::Result {
    let (start_minimized, size) = {
        let c = lock(&core);
        (
            c.config.general.start_minimized,
            c.config.general.window_size.unwrap_or([880.0, 600.0]),
        )
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(platform::MAIN_TITLE)
            .with_inner_size(size)
            .with_min_inner_size([760.0, 520.0])
            .with_decorations(false)
            .with_visible(!start_minimized)
            .with_icon(Arc::new(window_icon())),
        ..Default::default()
    };
    eframe::run_native(
        platform::MAIN_TITLE,
        options,
        Box::new(move |cc| Ok(Box::new(ReticleApp::new(cc, core, ctl)))),
    )
}

#[allow(dead_code)]
fn _keep(_: &Config) {}
