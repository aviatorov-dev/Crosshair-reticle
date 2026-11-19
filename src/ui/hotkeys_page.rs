//! "Hotkeys" page: click a field, press the combination. Conflicts are shown inline.

use std::collections::HashMap;

use eframe::egui::{self, RichText, Sense, Stroke, Ui, Vec2};

use super::widgets::{self, card};
use super::{tr, Env};
use crate::hotkeys;
use crate::overlay::Command;

pub fn show(ui: &mut Ui, env: &mut Env) {
    let l = env.lang;
    let pal = env.pal;
    widgets::heading(ui, tr(l, "Горячие клавиши", "Hotkeys"));
    widgets::muted(
        ui,
        &pal,
        tr(
            l,
            "Нажмите на поле и затем на нужное сочетание (с Ctrl, Alt или Shift). Esc - отмена.",
            "Click a field, then press the combination (with Ctrl, Alt or Shift). Esc cancels.",
        ),
    );
    ui.add_space(8.0);

    // rows: (action key, label, combo)
    let mut rows: Vec<(String, String, Option<String>)> = vec![
        (
            "toggle".into(),
            tr(l, "Показать / скрыть прицел", "Show / hide crosshair").into(),
            Some(env.core.config.hotkeys.toggle.clone()),
        ),
        (
            "open_settings".into(),
            tr(l, "Открыть настройки", "Open settings").into(),
            env.core.config.hotkeys.open_settings.clone(),
        ),
        (
            "next_preset".into(),
            tr(l, "Следующий пресет", "Next preset").into(),
            env.core.config.hotkeys.next_preset.clone(),
        ),
        (
            "prev_preset".into(),
            tr(l, "Предыдущий пресет", "Previous preset").into(),
            env.core.config.hotkeys.prev_preset.clone(),
        ),
    ];
    let global_count = rows.len();
    for p in &env.core.config.presets {
        rows.push((
            format!("preset:{}", p.id),
            format!("{}: {}", tr(l, "Пресет", "Preset"), p.name),
            p.hotkey.clone(),
        ));
    }

    // conflicts inside our own set
    let mut by_combo: HashMap<String, Vec<String>> = HashMap::new();
    for (_key, label, combo) in &rows {
        if let Some(c) = combo.as_deref().and_then(hotkeys::canonical) {
            by_combo
                .entry(c.to_lowercase())
                .or_default()
                .push(label.clone());
        }
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            card(ui, &pal, |ui| {
                egui::Grid::new("hotkey_grid")
                    .num_columns(3)
                    .spacing([14.0, 8.0])
                    .striped(false)
                    .show(ui, |ui| {
                        for (i, (key, label, combo)) in rows.iter().enumerate() {
                            if i == global_count {
                                ui.end_row();
                            }
                            ui.label(label);

                            let recording = env.st.recording.as_deref() == Some(key.as_str());
                            let canon = combo.as_deref().and_then(hotkeys::canonical);
                            let dup = canon
                                .as_ref()
                                .and_then(|c| by_combo.get(&c.to_lowercase()))
                                .map(|v| v.len() > 1)
                                .unwrap_or(false);
                            let busy = env.core.hotkey_errors.get(key).cloned();

                            let text = if recording {
                                tr(l, "Нажмите сочетание...", "Press a combination...").to_string()
                            } else {
                                canon
                                    .clone()
                                    .unwrap_or_else(|| tr(l, "не назначено", "not set").to_string())
                            };
                            let problem = dup || busy.is_some();
                            let color = if problem {
                                pal.danger
                            } else if recording {
                                pal.accent
                            } else {
                                pal.text
                            };
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(190.0, 26.0), Sense::click());
                            ui.painter().rect_filled(rect, 6.0, pal.bg);
                            ui.painter().rect_stroke(
                                rect,
                                6.0,
                                Stroke::new(
                                    if recording || problem {
                                        1.5_f32
                                    } else {
                                        1.0_f32
                                    },
                                    if recording {
                                        pal.accent
                                    } else if problem {
                                        pal.danger
                                    } else {
                                        pal.stroke
                                    },
                                ),
                            );
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                &text,
                                egui::FontId::monospace(13.0),
                                color,
                            );
                            let mut hover = String::new();
                            if dup {
                                hover.push_str(tr(
                                    l,
                                    "Конфликт: сочетание используется несколько раз. ",
                                    "Conflict: the combination is used more than once. ",
                                ));
                            }
                            if busy.is_some() {
                                hover.push_str(tr(
                                    l,
                                    "Сочетание занято другим приложением или системой.",
                                    "The combination is taken by another app or the system.",
                                ));
                            }
                            let resp = if hover.is_empty() {
                                resp
                            } else {
                                resp.on_hover_text(hover)
                            };
                            if resp.clicked() {
                                if recording {
                                    env.st.recording = None;
                                    env.ctl.send(Command::SuspendHotkeys(false));
                                } else {
                                    if env.st.recording.is_none() {
                                        env.ctl.send(Command::SuspendHotkeys(true));
                                    }
                                    env.st.recording = Some(key.clone());
                                }
                            }

                            ui.horizontal(|ui| {
                                let is_global = i < global_count;
                                let label = if is_global {
                                    tr(l, "По умолчанию", "Default")
                                } else {
                                    tr(l, "Убрать", "Clear")
                                };
                                if ui.small_button(label).clicked() {
                                    env.core.config.set_hotkey(key, None);
                                    env.changed = true;
                                }
                                if dup {
                                    ui.label(
                                        RichText::new(tr(l, "конфликт", "conflict"))
                                            .small()
                                            .color(pal.danger),
                                    );
                                } else if busy.is_some() {
                                    ui.label(
                                        RichText::new(tr(l, "занято системой", "in use"))
                                            .small()
                                            .color(pal.warn),
                                    );
                                }
                            });
                            ui.end_row();
                        }
                    });
            });
        });
}
