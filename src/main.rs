//! Crosshair reticle entry point.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    use std::sync::{Arc, Mutex};

    use crosshair_reticle::config::Config;
    use crosshair_reticle::core::{Core, ToastKind};
    use crosshair_reticle::{autostart, overlay, platform, ui, util};

    platform::set_dpi_awareness();

    let paths = match util::Paths::default_dirs() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cannot prepare the data folder: {e:#}");
            return;
        }
    };
    let _log_guard = util::init_logging(&paths);
    std::panic::set_hook(Box::new(|info| tracing::error!("panic: {info}")));
    tracing::info!("Crosshair reticle {} starting", env!("CARGO_PKG_VERSION"));

    // a second launch just brings the first window to the front
    let _instance = match platform::single_instance() {
        Some(g) => g,
        None => {
            platform::show_settings_window();
            return;
        }
    };

    let loaded = Config::load(&paths);
    if loaded.config.general.autostart {
        // keeps the registry path correct if the exe was moved
        if let Err(e) = autostart::set_enabled(true) {
            tracing::warn!("autostart: {e}");
        }
    }
    let mut core = Core::new(loaded.config, paths);
    if let Some(n) = loaded.notice {
        core.toast(
            ToastKind::Warning,
            format!("Конфиг повреждён и сброшен (копия: config.toml.corrupt.bak) / Config was corrupt and reset: {n}"),
        );
    }
    let core = Arc::new(Mutex::new(core));

    let (ctl, rx) = overlay::Controller::new();
    let _platform = overlay::spawn(core.clone(), ctl.clone(), rx);

    match ui::run(core.clone(), ctl.clone()) {
        Ok(()) => {}
        Err(e) => tracing::error!("GUI stopped: {e}"),
    }
    // The settings window is gone (or never started). The overlay and tray keep working on their
    // own thread until "Quit" is chosen in the tray menu, which exits the process.
    crosshair_reticle::core::lock(&core).save_now();
    loop {
        std::thread::park();
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Crosshair reticle runs on Windows only. Use `cargo test` on this platform to run the platform independent tests.");
}
