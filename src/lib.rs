//! Crosshair reticle - a customizable crosshair overlay for Windows.
//!
//! Platform independent modules (config, render, hotkeys, core state) are always built and tested;
//! everything that touches Win32 or the GUI lives behind `cfg(windows)`.

pub mod brand;
pub mod config;
pub mod core;
pub mod hotkeys;
pub mod render;
pub mod util;

#[cfg(windows)]
pub mod autostart;
#[cfg(windows)]
pub mod overlay;
#[cfg(windows)]
pub mod platform;
#[cfg(windows)]
pub mod tray;
#[cfg(windows)]
pub mod ui;
