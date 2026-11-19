//! Tray icon + menu. Must be created and polled on the overlay thread (it owns the Win32
//! message loop), so it keeps working while the settings window is hidden.

use tray_icon::menu::{
    CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::brand::icon_rgba;
use crate::config::Lang;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayAction {
    Toggle,
    OpenSettings,
    Quit,
    Preset(String),
}

/// Everything the menu shows; the menu is rebuilt only when this changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayState {
    pub visible: bool,
    pub lang: Lang,
    pub active_id: String,
    pub active_name: String,
    pub presets: Vec<(String, String)>,
}

pub struct TrayUi {
    icon: TrayIcon,
    on: Icon,
    off: Icon,
    last: Option<TrayState>,
}

fn make_icon(on: bool) -> Icon {
    Icon::from_rgba(icon_rgba(32, on), 32, 32).expect("32x32 RGBA icon is valid")
}

fn t(lang: Lang, ru: &'static str, en: &'static str) -> &'static str {
    match lang {
        Lang::Ru => ru,
        Lang::En => en,
    }
}

impl TrayUi {
    pub fn new() -> anyhow::Result<Self> {
        let on = make_icon(true);
        let off = make_icon(false);
        let icon = TrayIconBuilder::new()
            .with_icon(on.clone())
            .with_tooltip("Crosshair reticle")
            .with_menu_on_left_click(false)
            .build()?;
        Ok(TrayUi {
            icon,
            on,
            off,
            last: None,
        })
    }

    pub fn update(&mut self, st: &TrayState) {
        if self.last.as_ref() == Some(st) {
            return;
        }
        let lang = st.lang;
        let toggle_text = if st.visible {
            t(lang, "Скрыть прицел", "Hide crosshair")
        } else {
            t(lang, "Показать прицел", "Show crosshair")
        };
        let toggle = MenuItem::with_id("toggle", toggle_text, true, None);
        let settings = MenuItem::with_id(
            "settings",
            t(lang, "Настройки...", "Settings..."),
            true,
            None,
        );
        let quit = MenuItem::with_id("quit", t(lang, "Выход", "Quit"), true, None);

        let sub = Submenu::new(t(lang, "Пресеты", "Presets"), true);
        for (id, name) in &st.presets {
            let item = CheckMenuItem::with_id(
                MenuId::new(format!("preset:{id}")),
                name,
                true,
                *id == st.active_id,
                None,
            );
            let _ = sub.append(&item);
        }

        let menu = Menu::new();
        let _ = menu.append(&toggle);
        let _ = menu.append(&sub);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&settings);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit);
        self.icon.set_menu(Some(Box::new(menu)));

        let _ = self.icon.set_icon(Some(if st.visible {
            self.on.clone()
        } else {
            self.off.clone()
        }));
        let _ = self
            .icon
            .set_tooltip(Some(format!("Crosshair reticle - {}", st.active_name)));
        self.last = Some(st.clone());
    }

    /// Drains pending tray / menu events.
    pub fn poll(&self) -> Vec<TrayAction> {
        let mut out = Vec::new();
        while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
            match ev {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => out.push(TrayAction::Toggle),
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => out.push(TrayAction::OpenSettings),
                _ => {}
            }
        }
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            let id = ev.id.0.as_str();
            match id {
                "toggle" => out.push(TrayAction::Toggle),
                "settings" => out.push(TrayAction::OpenSettings),
                "quit" => out.push(TrayAction::Quit),
                _ => {
                    if let Some(p) = id.strip_prefix("preset:") {
                        out.push(TrayAction::Preset(p.to_string()));
                    }
                }
            }
        }
        // a double click is reported as Click, DoubleClick, Click: keep the toggles balanced
        if out.contains(&TrayAction::OpenSettings) {
            let toggles = out.iter().filter(|a| **a == TrayAction::Toggle).count();
            if toggles % 2 == 0 {
                out.retain(|a| *a != TrayAction::Toggle);
            }
        }
        out
    }
}
