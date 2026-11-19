//! First-run guide: pick a look, check the position, learn the hotkeys.

use eframe::egui::{self, Align2, Ui};

use super::widgets::{self, primary_button};
use super::{tr, Env, Page};
use crate::config::builtin_presets;
use crate::overlay::Command;

pub fn show(ctx: &egui::Context, env: &mut Env) {
    let Some(step) = env.st.onboarding else {
        return;
    };
    let l = env.lang;
    let pal = env.pal;
    let mut next: Option<u8> = None;
    let mut finish = false;
    let mut skip = false;

    egui::Window::new(format!("{} {}/3", tr(l, "Быстрый старт", "Quick start"), step + 1))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([440.0, 0.0])
        .show(ctx, |ui: &mut Ui| {
            match step {
                0 => {
                    ui.label(tr(l, "Выберите стартовый прицел. Его можно изменить в любой момент.", "Pick a starting crosshair. You can change it any time."));
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        for p in builtin_presets().iter().take(8) {
                            let active = env.core.config.active_preset == p.id;
                            if ui.selectable_label(active, &p.name).clicked() {
                                env.core.config.set_active(&p.id);
                                env.changed = true;
                            }
                        }
                    });
                }
                1 => {
                    ui.label(tr(
                        l,
                        "Прицел рисуется по центру выбранного монитора. Нажмите кнопку, чтобы увидеть рамку монитора и центр. Сдвиг можно подправить на странице «Прицел».",
                        "The crosshair is drawn at the centre of the chosen monitor. Press the button to see the monitor frame and centre. Fine-tune the offset on the Crosshair page.",
                    ));
                    ui.add_space(6.0);
                    if ui.button(tr(l, "Показать рамку на 3 секунды", "Show frame for 3 seconds")).clicked() {
                        env.ctl.send(Command::FlashFrame);
                    }
                }
                _ => {
                    ui.label(tr(l, "Главные горячие клавиши:", "Main hotkeys:"));
                    ui.add_space(4.0);
                    let hk = &env.core.config.hotkeys;
                    for (name, combo) in [
                        (
                            tr(l, "Показать / скрыть", "Show / hide"),
                            Some(hk.toggle.clone()),
                        ),
                        (tr(l, "Настройки", "Settings"), hk.open_settings.clone()),
                        (
                            tr(l, "Следующий пресет", "Next preset"),
                            hk.next_preset.clone(),
                        ),
                        (
                            tr(l, "Предыдущий пресет", "Previous preset"),
                            hk.prev_preset.clone(),
                        ),
                    ] {
                        ui.horizontal(|ui| {
                            ui.add_sized([170.0, 20.0], egui::Label::new(name));
                            ui.label(
                                egui::RichText::new(
                                    combo
                                        .unwrap_or_else(|| tr(l, "не назначено", "not set").into()),
                                )
                                .monospace(),
                            );
                        });
                    }
                    ui.add_space(4.0);
                    widgets::muted(ui, &pal, tr(l, "Окно можно закрыть - Crosshair reticle продолжит работать в трее.", "You can close this window - Crosshair reticle keeps running in the tray."));
                }
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(tr(l, "Пропустить", "Skip")).clicked() {
                    skip = true;
                }
                widgets::right_aligned(ui, |ui| {
                    if step < 2 {
                        if primary_button(ui, &pal, tr(l, "Далее", "Next")).clicked() {
                            next = Some(step + 1);
                        }
                    } else if primary_button(ui, &pal, tr(l, "Готово", "Done")).clicked() {
                        finish = true;
                    }
                });
            });
        });

    if let Some(n) = next {
        env.st.onboarding = Some(n);
    }
    if finish {
        // give the user an editable copy of what they picked
        let active = env.core.config.active().clone();
        if active.builtin {
            let name = tr(l, "Мой прицел", "My crosshair");
            let id = env.core.config.add_preset(name, active.crosshair);
            env.core.config.set_active(&id);
        }
        env.st.page = Page::Editor;
    }
    if finish || skip {
        env.core.config.general.onboarded = true;
        env.st.onboarding = None;
        env.changed = true;
    }
}
