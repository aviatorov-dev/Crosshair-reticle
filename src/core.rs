//! State shared between the GUI thread and the platform (overlay / tray / hotkey) thread.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use image::RgbaImage;

use crate::config::{Config, MonitorChoice};
use crate::util::{fnv1a, Paths};

pub type Shared = Arc<Mutex<Core>>;

/// Poison-tolerant lock: a panic in one thread must not take the other one down.
pub fn lock(shared: &Shared) -> MutexGuard<'_, Core> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorInfo {
    /// Windows device name, e.g. `\\.\DISPLAY1`.
    pub name: String,
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
    pub dpi: u32,
}

impl MonitorInfo {
    pub fn dpi_scale(&self) -> f32 {
        self.dpi.max(96) as f32 / 96.0
    }
}

/// Picks the monitor for a choice; falls back to the primary one (or the first).
pub fn pick_monitor<'a>(
    monitors: &'a [MonitorInfo],
    choice: &MonitorChoice,
) -> Option<&'a MonitorInfo> {
    if let MonitorChoice::Named(n) = choice {
        if let Some(m) = monitors.iter().find(|m| &m.name == n) {
            return Some(m);
        }
    }
    monitors
        .iter()
        .find(|m| m.primary)
        .or_else(|| monitors.first())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: ToastKind,
    pub text: String,
    pub created: Instant,
}

/// Lazily loads PNGs from the images folder.
#[derive(Default)]
pub struct ImageCache {
    map: HashMap<String, Option<Arc<RgbaImage>>>,
}

impl ImageCache {
    pub fn get(&mut self, paths: &Paths, file: &str) -> Option<Arc<RgbaImage>> {
        if file.is_empty() || file.contains(['/', '\\']) {
            return None;
        }
        self.map
            .entry(file.to_string())
            .or_insert_with(|| {
                image::open(paths.images_dir.join(file))
                    .ok()
                    .map(|i| Arc::new(i.to_rgba8()))
            })
            .clone()
    }

    pub fn forget(&mut self, file: &str) {
        self.map.remove(file);
    }
}

pub struct Core {
    pub config: Config,
    pub paths: Paths,
    /// Overlay visibility (runtime only, not persisted).
    pub visible: bool,
    pub monitors: Vec<MonitorInfo>,
    /// Action key -> message, for hotkeys the system refused to register.
    pub hotkey_errors: HashMap<String, String>,
    pub toasts: Vec<Toast>,
    pub images: ImageCache,
    /// Set by the GUI so the platform thread can wake it.
    pub repaint: Option<Box<dyn Fn() + Send + Sync>>,
    dirty: bool,
    last_change: Instant,
}

const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

impl Core {
    pub fn new(config: Config, paths: Paths) -> Self {
        Core {
            config,
            paths,
            visible: true,
            monitors: Vec::new(),
            hotkey_errors: HashMap::new(),
            toasts: Vec::new(),
            images: ImageCache::default(),
            repaint: None,
            dirty: false,
            last_change: Instant::now(),
        }
    }

    /// Marks the config as modified; it is written after a short quiet period.
    pub fn touch(&mut self) {
        self.dirty = true;
        self.last_change = Instant::now();
    }

    pub fn save_if_due(&mut self) {
        if self.dirty && self.last_change.elapsed() >= SAVE_DEBOUNCE {
            self.save_now();
        }
    }

    pub fn save_now(&mut self) {
        if !self.dirty {
            return;
        }
        match self.config.save(&self.paths) {
            Ok(()) => self.dirty = false,
            Err(e) => {
                tracing::error!("saving config failed: {e:#}");
                // retry later, but do not spam toasts every tick
                self.last_change = Instant::now();
            }
        }
    }

    pub fn request_repaint(&self) {
        if let Some(f) = &self.repaint {
            f();
        }
    }

    pub fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.toasts.push(Toast {
            kind,
            text: text.into(),
            created: Instant::now(),
        });
        self.toasts.truncate(6);
        self.request_repaint();
    }

    // ------------------------------------------------ actions shared by hotkeys, tray and GUI

    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
    }

    pub fn cycle_preset(&mut self, dir: i32) {
        self.config.cycle_active(dir);
        self.touch();
    }

    pub fn activate_preset(&mut self, id: &str) {
        if self.config.set_active(id) {
            self.touch();
        }
    }

    // ------------------------------------------------------------------- images

    /// Stores a PNG under a content-hash name and returns the file name.
    pub fn import_png_bytes(&mut self, bytes: &[u8]) -> Result<String> {
        if !matches!(image::guess_format(bytes), Ok(image::ImageFormat::Png)) {
            bail!("only PNG images are supported");
        }
        let img = image::load_from_memory(bytes).context("cannot decode PNG")?;
        if img.width() > 2048 || img.height() > 2048 {
            bail!("image is too large (max 2048x2048)");
        }
        if img.width() == 0 || img.height() == 0 {
            bail!("empty image");
        }
        let name = format!("{:016x}.png", fnv1a(bytes));
        std::fs::create_dir_all(&self.paths.images_dir)?;
        std::fs::write(self.paths.images_dir.join(&name), bytes).context("cannot store image")?;
        self.images.forget(&name);
        Ok(name)
    }

    pub fn import_png_file(&mut self, path: &Path) -> Result<String> {
        let bytes =
            std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
        self.import_png_bytes(&bytes)
    }

    pub fn image_bytes(&self, file: &str) -> Option<Vec<u8>> {
        if file.contains(['/', '\\']) {
            return None;
        }
        std::fs::read(self.paths.images_dir.join(file)).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, SharedPreset};

    fn tmp(name: &str) -> Paths {
        let dir = std::env::temp_dir().join(format!("reticle-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = Paths::new(dir);
        p.ensure().unwrap();
        p
    }

    fn png_bytes() -> Vec<u8> {
        let img = RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    #[test]
    fn config_roundtrip_and_defaults() {
        let paths = tmp("roundtrip");
        let loaded = Config::load(&paths);
        assert!(loaded.notice.is_none());
        let mut cfg = loaded.config;
        assert!(cfg.presets.len() >= 8);
        cfg.general.autostart = true;
        cfg.general.hide_crosshair_on_rmb = true;
        let id = cfg.add_preset("Мой прицел", Default::default());
        cfg.set_active(&id);
        cfg.save(&paths).unwrap();
        let again = Config::load(&paths).config;
        assert_eq!(again, cfg);
    }

    #[test]
    fn corrupt_config_is_backed_up_and_reset() {
        let paths = tmp("corrupt");
        std::fs::write(&paths.config_file, "this is = [not valid").unwrap();
        let loaded = Config::load(&paths);
        assert!(loaded.notice.is_some());
        assert!(paths
            .config_file
            .with_extension("toml.corrupt.bak")
            .exists());
        assert_eq!(loaded.config, Config::default());
        // the reset file is valid now
        assert!(Config::load(&paths).notice.is_none());
    }

    #[test]
    fn old_config_without_version_is_migrated_with_backup() {
        let paths = tmp("migrate");
        std::fs::write(
            &paths.config_file,
            "active_preset = \"red-cross\"\n[general]\nautostart = true\n",
        )
        .unwrap();
        let loaded = Config::load(&paths);
        assert!(loaded.notice.is_none());
        assert!(loaded.config.general.autostart);
        assert!(loaded.config.general.show_crosshair);
        assert!(!loaded.config.general.hide_crosshair_on_rmb);
        assert_eq!(
            loaded.config.version,
            crate::config::migrate::CURRENT_VERSION
        );
        assert!(paths.backup_file.exists());
    }

    #[test]
    fn hotkey_defaults_are_migrated_but_custom_bindings_are_kept() {
        let text = r#"
            version = 1
            [hotkeys]
            toggle = "Ctrl+Alt+X"
            open_settings = "Ctrl+Alt+S"
            next_preset = "Ctrl+Alt+Q"
            prev_preset = "Ctrl+Alt+Left"
        "#;
        let (cfg, migrated) = Config::parse(text).unwrap();
        assert!(migrated);
        assert_eq!(cfg.hotkeys.toggle, "Ctrl+Alt+X");
        assert_eq!(cfg.hotkeys.open_settings, None);
        assert_eq!(cfg.hotkeys.next_preset.as_deref(), Some("Ctrl+Alt+Q"));
        assert_eq!(cfg.hotkeys.prev_preset, None);
    }

    #[test]
    fn newer_config_version_is_rejected_safely() {
        let paths = tmp("newer");
        std::fs::write(&paths.config_file, "version = 99\n").unwrap();
        let loaded = Config::load(&paths);
        assert!(loaded.notice.is_some());
    }

    #[test]
    fn validate_repairs_bad_values() {
        let text = r##"
            active_preset = "nope"
            [hotkeys]
            toggle = "garbage"
            [general]
            ui_scale = 99.0
            [[presets]]
            id = "a"
            name = "A"
            hotkey = "not a hotkey"
            [presets.crosshair]
            thickness = 9999.0
            opacity = -3.0
        "##;
        let (cfg, _) = Config::parse(text).unwrap();
        assert_eq!(cfg.hotkeys.toggle, "Ctrl+Alt+X");
        assert_eq!(cfg.general.ui_scale, 1.5);
        let a = cfg.preset("a").unwrap();
        assert_eq!(a.hotkey, None);
        assert_eq!(a.crosshair.thickness, 64.0);
        assert_eq!(a.crosshair.opacity, 0.0);
        assert!(cfg.preset(&cfg.active_preset).is_some());
        assert!(cfg.presets.len() >= 9, "built-ins are merged in");
    }

    #[test]
    fn preset_operations() {
        let mut cfg = Config::default();
        let first = cfg.presets[0].id.clone();
        assert!(
            cfg.delete_preset(&first).is_none(),
            "built-ins cannot be deleted"
        );
        let id = cfg.add_preset("Test", Default::default());
        let id2 = cfg.add_preset("Test", Default::default());
        assert_ne!(id, id2);
        cfg.set_active(&id);
        let (removed, idx) = cfg.delete_preset(&id).unwrap();
        assert_ne!(cfg.active_preset, id);
        cfg.restore_preset(removed, idx);
        assert!(cfg.preset(&id).is_some());
        let n = cfg.presets.len();
        cfg.set_active(&cfg.presets[n - 1].id.clone());
        cfg.cycle_active(1);
        assert_eq!(cfg.active_preset, cfg.presets[0].id);
        cfg.cycle_active(-1);
        assert_eq!(cfg.active_preset, cfg.presets[n - 1].id);
    }

    #[test]
    fn reset_builtin_restores_original() {
        let mut cfg = Config::default();
        let id = cfg.presets[0].id.clone();
        let original = cfg.presets[0].crosshair.clone();
        cfg.preset_mut(&id).unwrap().crosshair.thickness = 10.0;
        cfg.reset_preset(&id);
        assert_eq!(cfg.preset(&id).unwrap().crosshair, original);
    }

    #[test]
    fn hotkey_bindings_include_preset_hotkeys() {
        let mut cfg = Config::default();
        assert_eq!(cfg.hotkeys.open_settings, None);
        assert_eq!(cfg.hotkeys.next_preset, None);
        assert_eq!(cfg.hotkeys.prev_preset, None);
        assert_eq!(
            cfg.hotkey_bindings(),
            vec![("toggle".into(), "Ctrl+Alt+X".into())]
        );
        cfg.set_hotkey("open_settings", Some("Ctrl+Alt+S".into()));
        assert_eq!(cfg.hotkeys.open_settings.as_deref(), Some("Ctrl+Alt+S"));
        cfg.set_hotkey("open_settings", None);
        assert_eq!(cfg.hotkeys.open_settings, None);

        let id = cfg.presets[1].id.clone();
        cfg.set_hotkey(&format!("preset:{id}"), Some("Ctrl+Alt+2".into()));
        let b = cfg.hotkey_bindings();
        assert!(b.contains(&(format!("preset:{id}"), "Ctrl+Alt+2".to_string())));
        cfg.set_hotkey("toggle", None);
        assert_eq!(cfg.hotkeys.toggle, "Ctrl+Alt+X");
    }

    #[test]
    fn share_roundtrip_json_and_string() {
        let cfg = Config::default();
        let p = cfg.presets[0].clone();
        let shared = SharedPreset::new(&p, Some(vec![1, 2, 3]));
        for text in [shared.to_json(), shared.to_share_string()] {
            let back = SharedPreset::parse(&text).unwrap();
            assert_eq!(back.preset.crosshair, p.crosshair);
            assert!(!back.preset.builtin);
            assert_eq!(back.png_bytes().unwrap(), vec![1, 2, 3]);
        }
        assert!(SharedPreset::parse("hello").is_err());
        assert!(SharedPreset::parse("RETICLE1:!!!").is_err());
        assert!(SharedPreset::parse("{\"x\":1}").is_err());
    }

    #[test]
    fn image_import_and_cache() {
        let paths = tmp("images");
        let mut core = Core::new(Config::default(), paths.clone());
        let name = core.import_png_bytes(&png_bytes()).unwrap();
        assert!(name.ends_with(".png"));
        let img = core.images.get(&paths, &name).unwrap();
        assert_eq!((img.width(), img.height()), (4, 4));
        assert!(core.import_png_bytes(b"not a png").is_err());
        assert!(core.images.get(&paths, "../evil.png").is_none());
    }

    #[test]
    fn debounced_save() {
        let paths = tmp("debounce");
        let mut core = Core::new(Config::default(), paths.clone());
        core.config.general.autostart = true;
        core.touch();
        core.save_if_due();
        assert!(!paths.config_file.exists(), "not yet: debounce");
        core.save_now();
        assert!(paths.config_file.exists());
    }

    #[test]
    fn monitor_choice_falls_back_to_primary() {
        let m = |name: &str, primary| MonitorInfo {
            name: name.into(),
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
            primary,
            dpi: 96,
        };
        let list = vec![m("A", false), m("B", true)];
        assert_eq!(
            pick_monitor(&list, &MonitorChoice::Primary).unwrap().name,
            "B"
        );
        assert_eq!(
            pick_monitor(&list, &MonitorChoice::Named("A".into()))
                .unwrap()
                .name,
            "A"
        );
        assert_eq!(
            pick_monitor(&list, &MonitorChoice::Named("gone".into()))
                .unwrap()
                .name,
            "B"
        );
        assert!(pick_monitor(&[], &MonitorChoice::Primary).is_none());
    }
}
