//! "Settings" page: monitor, startup, appearance, data, about.

use eframe::egui::{self, Ui};

use super::widgets::{self, bool_row, card, danger_button, slider_row};
use super::{tr, Env};
use crate::autostart;
use crate::config::{Config, Lang, MonitorChoice, ThemeMode};
use crate::core::ToastKind;
use crate::overlay::Command;
use crate::platform;

pub fn show(ui: &mut Ui, env: &mut Env) {
    let l = env.lang;
    let pal = env.pal;
    widgets::heading(ui, tr(l, "Настройки", "Settings"));
    ui.add_space(8.0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        // ------------------------------------------------------------ display
        card(ui, &pal, |ui| {
            ui.label(egui::RichText::new(tr(l, "Экран", "Display")).strong());
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_sized([widgets::LABEL_W, 22.0], egui::Label::new(tr(l, "Монитор", "Monitor")));
                let current = match &env.core.config.general.monitor {
                    MonitorChoice::Primary => tr(l, "Основной", "Primary").to_string(),
                    MonitorChoice::Named(n) => env
                        .core
                        .monitors
                        .iter()
                        .position(|m| &m.name == n)
                        .map(|i| monitor_label(env, i))
                        .unwrap_or_else(|| format!("{n} ({})", tr(l, "не найден", "not found"))),
                };
                let mut choice = env.core.config.general.monitor.clone();
                egui::ComboBox::from_id_salt("monitor").selected_text(current).width(260.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut choice, MonitorChoice::Primary, tr(l, "Основной", "Primary"));
                    for (i, m) in env.core.monitors.iter().enumerate() {
                        let label = monitor_label_of(l, i, m);
                        ui.selectable_value(&mut choice, MonitorChoice::Named(m.name.clone()), label);
                    }
                });
                if choice != env.core.config.general.monitor {
                    env.core.config.general.monitor = choice;
                    env.changed = true;
                }
            });
            let mut dpi = env.core.config.general.scale_with_dpi;
            if bool_row(ui, &pal, tr(l, "Масштабировать по DPI", "Scale with DPI"), &mut dpi) {
                env.core.config.general.scale_with_dpi = dpi;
                env.changed = true;
            }
            let mut show_crosshair = env.core.config.general.show_crosshair;
            if bool_row(
                ui,
                &pal,
                tr(l, "Показывать прицел", "Show crosshair"),
                &mut show_crosshair,
            ) {
                env.core.config.general.show_crosshair = show_crosshair;
                env.changed = true;
            }
            ui.add_enabled_ui(show_crosshair, |ui| {
                let mut hide_on_rmb = env.core.config.general.hide_crosshair_on_rmb;
                if bool_row(
                    ui,
                    &pal,
                    tr(
                        l,
                        "Скрывать, пока зажата ПКМ",
                        "Hide while right mouse button is held",
                    ),
                    &mut hide_on_rmb,
                ) {
                    env.core.config.general.hide_crosshair_on_rmb = hide_on_rmb;
                    env.changed = true;
                }
            });
            widgets::muted(
                ui,
                &pal,
                tr(
                    l,
                    "Прицел всегда по центру выбранного монитора. Работает в оконном и безрамочном режиме; в эксклюзивном полноэкранном оверлей может не отображаться.",
                    "The crosshair is always centred on the chosen monitor. Works in windowed and borderless mode; it may not show over exclusive fullscreen.",
                ),
            );
            if ui.button(tr(l, "Показать рамку на 3 секунды", "Show frame for 3 seconds")).clicked() {
                env.ctl.send(Command::FlashFrame);
            }
        });

        // ------------------------------------------------------------ startup
        ui.add_space(8.0);
        card(ui, &pal, |ui| {
            ui.label(egui::RichText::new(tr(l, "Запуск", "Startup")).strong());
            ui.add_space(4.0);
            let mut auto = env.core.config.general.autostart;
            if bool_row(ui, &pal, tr(l, "Запускать с Windows", "Start with Windows"), &mut auto) {
                match autostart::set_enabled(auto) {
                    Ok(()) => {
                        env.core.config.general.autostart = auto;
                        env.changed = true;
                    }
                    Err(e) => env.toast(ToastKind::Error, format!("{e}")),
                }
            }
            let mut min = env.core.config.general.start_minimized;
            if bool_row(ui, &pal, tr(l, "Сразу в трей", "Start minimised to tray"), &mut min) {
                env.core.config.general.start_minimized = min;
                env.changed = true;
            }
        });

        // --------------------------------------------------------- appearance
        ui.add_space(8.0);
        card(ui, &pal, |ui| {
            ui.label(egui::RichText::new(tr(l, "Оформление", "Appearance")).strong());
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_sized([widgets::LABEL_W, 22.0], egui::Label::new(tr(l, "Тема", "Theme")));
                let mut theme = env.core.config.general.theme;
                let items = vec![
                    (ThemeMode::Dark, tr(l, "Тёмная", "Dark").to_string()),
                    (ThemeMode::Light, tr(l, "Светлая", "Light").to_string()),
                    (ThemeMode::System, tr(l, "Системная", "System").to_string()),
                ];
                if widgets::segmented(ui, &pal, &items, &mut theme) {
                    env.core.config.general.theme = theme;
                    env.changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.add_sized([widgets::LABEL_W, 22.0], egui::Label::new(tr(l, "Язык", "Language")));
                let mut lang = env.core.config.general.language;
                let items = vec![(Lang::Ru, "Русский".to_string()), (Lang::En, "English".to_string())];
                if widgets::segmented(ui, &pal, &items, &mut lang) {
                    env.core.config.general.language = lang;
                    env.changed = true;
                }
            });
            let mut pct = env.core.config.general.ui_scale * 100.0;
            if slider_row(ui, tr(l, "Масштаб интерфейса", "Interface scale"), &mut pct, 80.0..=150.0, 5.0, 100.0, "%", 0) {
                env.core.config.general.ui_scale = pct / 100.0;
                env.changed = true;
            }
            let mut rm = env.core.config.general.reduce_motion;
            if bool_row(ui, &pal, tr(l, "Меньше анимаций", "Reduce motion"), &mut rm) {
                env.core.config.general.reduce_motion = rm;
                env.changed = true;
            }
        });

        // --------------------------------------------------------------- data
        ui.add_space(8.0);
        card(ui, &pal, |ui| {
            ui.label(egui::RichText::new(tr(l, "Данные", "Data")).strong());
            ui.add_space(4.0);
            widgets::muted(ui, &pal, &env.core.paths.root.display().to_string());
            ui.horizontal(|ui| {
                if ui.button(tr(l, "Открыть папку с конфигом", "Open config folder")).clicked() {
                    platform::open_folder(&env.core.paths.root);
                }
                if ui.button(tr(l, "Открыть папку логов", "Open logs folder")).clicked() {
                    platform::open_folder(&env.core.paths.logs_dir);
                }
            });
            ui.add_space(4.0);
            if env.st.confirm_reset {
                ui.label(
                    egui::RichText::new(tr(
                        l,
                        "Все настройки и ваши пресеты будут удалены. Продолжить?",
                        "All settings and your presets will be removed. Continue?",
                    ))
                    .color(pal.danger),
                );
                ui.horizontal(|ui| {
                    if danger_button(ui, &pal, tr(l, "Да, сбросить всё", "Yes, reset everything")).clicked() {
                        let mut fresh = Config::default();
                        fresh.general.onboarded = true;
                        fresh.general.language = env.core.config.general.language;
                        env.core.config = fresh;
                        let _ = autostart::set_enabled(false);
                        env.st.thumbs.clear();
                        env.st.confirm_reset = false;
                        env.changed = true;
                    }
                    if ui.button(tr(l, "Отмена", "Cancel")).clicked() {
                        env.st.confirm_reset = false;
                    }
                });
            } else if danger_button(ui, &pal, tr(l, "Сбросить все настройки...", "Reset all settings...")).clicked() {
                env.st.confirm_reset = true;
            }
        });

        // -------------------------------------------------------------- about
        ui.add_space(8.0);
        card(ui, &pal, |ui| {
            ui.label(egui::RichText::new(format!("Crosshair reticle {}", env.versions())).strong());
            widgets::muted(
                ui,
                &pal,
                tr(
                    l,
                    "Внешний оверлей: не внедряется в процессы игр, не читает их память. Некоторые античиты всё же могут запрещать любые оверлеи - используйте на свой риск.",
                    "An external overlay: it never injects into or reads game processes. Some anti-cheats may still forbid any overlay - use at your own risk.",
                ),
            );
            widgets::muted(ui, &pal, "Rust, egui / eframe, tiny-skia, tray-icon, windows-rs (MIT / Apache-2.0)");
        });
        ui.add_space(12.0);
    });
}

fn monitor_label(env: &Env, i: usize) -> String {
    monitor_label_of(env.lang, i, &env.core.monitors[i])
}

fn monitor_label_of(l: Lang, i: usize, m: &crate::core::MonitorInfo) -> String {
    format!(
        "{} {} - {}x{}, {}%{}",
        tr(l, "Монитор", "Monitor"),
        i + 1,
        m.width,
        m.height,
        (m.dpi_scale() * 100.0).round() as u32,
        if m.primary {
            format!(" ({})", tr(l, "основной", "primary"))
        } else {
            String::new()
        }
    )
}

impl Env<'_> {
    fn versions(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}
