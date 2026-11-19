//! "Presets" page: grid of cards with thumbnails, search, import / export, rename, delete + undo.

use std::time::Instant;

use eframe::egui::{
    self, Align2, Color32, ColorImage, Pos2, Rect, RichText, Sense, Stroke, TextureOptions, Ui,
    Vec2,
};

use super::widgets::{self, primary_button};
use super::{tr, Action, Env, Page, UiToast, Undo};
use crate::config::Shape;
use crate::core::ToastKind;
use crate::render;

const CARD_H: f32 = 132.0;
const THUMB: f32 = 72.0;

pub fn show(ui: &mut Ui, env: &mut Env) {
    let l = env.lang;
    let pal = env.pal;

    ui.horizontal(|ui| {
        widgets::heading(ui, tr(l, "Пресеты", "Presets"));
        widgets::right_aligned(ui, |ui| {
            if ui
                .button(tr(l, "Вставить", "Paste"))
                .on_hover_text(tr(l, "Импорт из буфера обмена", "Import from clipboard"))
                .clicked()
            {
                env.actions.push(Action::PasteClipboard);
            }
            if ui.button(tr(l, "Импорт...", "Import...")).clicked() {
                env.actions.push(Action::ImportFile);
            }
            if primary_button(ui, &pal, tr(l, "+ Новый", "+ New")).clicked() {
                let base = env.core.config.active().crosshair.clone();
                let name = tr(l, "Новый пресет", "New preset");
                let id = env.core.config.add_preset(name, base);
                env.core.config.set_active(&id);
                env.st.rename = Some((id, name.to_string()));
                env.changed = true;
            }
            ui.add(
                egui::TextEdit::singleline(&mut env.st.search)
                    .hint_text(tr(l, "Поиск...", "Search..."))
                    .desired_width(150.0),
            );
        });
    });
    ui.add_space(8.0);

    let query = env.st.search.trim().to_lowercase();
    let ids: Vec<String> = env
        .core
        .config
        .presets
        .iter()
        .filter(|p| query.is_empty() || p.name.to_lowercase().contains(&query))
        .map(|p| p.id.clone())
        .collect();

    if ids.is_empty() {
        widgets::muted(ui, &pal, tr(l, "Ничего не найдено", "Nothing found"));
        return;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let gap = 10.0;
            let avail = ui.available_width();
            let cols = (((avail + gap) / (190.0 + gap)).floor() as usize).max(1);
            let w = (avail - gap * (cols as f32 - 1.0)) / cols as f32 - 1.0;
            egui::Grid::new("preset_grid")
                .spacing([gap, gap])
                .show(ui, |ui| {
                    for (n, id) in ids.iter().enumerate() {
                        card_ui(ui, env, id, w);
                        if (n + 1) % cols == 0 {
                            ui.end_row();
                        }
                    }
                });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .button(tr(
                        l,
                        "Экспортировать мои пресеты...",
                        "Export my presets...",
                    ))
                    .clicked()
                {
                    env.actions.push(Action::ExportAll);
                }
            });
        });
}

fn thumbnail(ui: &Ui, env: &mut Env, id: &str) -> Option<egui::TextureId> {
    let c = env.core.config.preset(id)?.crosshair.clone();
    if let Some((old, tex)) = env.st.thumbs.get(id) {
        if *old == c {
            return Some(tex.id());
        }
    }
    let paths = env.core.paths.clone();
    let bitmap = c
        .image
        .as_ref()
        .filter(|_| c.shape == Shape::Image)
        .and_then(|i| env.core.images.get(&paths, &i.file));
    // fit big crosshairs into the thumbnail instead of clipping them
    let natural = render::canvas_size(&c, 1.0, bitmap.as_deref()) as f32;
    let scale = (THUMB / natural).min(1.0);
    let pm = render::render(&c, scale, bitmap.as_deref());
    let img =
        ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
    let tex = ui
        .ctx()
        .load_texture(format!("thumb-{id}"), img, TextureOptions::NEAREST);
    let tid = tex.id();
    env.st.thumbs.insert(id.to_string(), (c, tex));
    Some(tid)
}

fn card_ui(ui: &mut Ui, env: &mut Env, id: &str, width: f32) {
    let l = env.lang;
    let pal = env.pal;
    let Some(preset) = env.core.config.preset(id).cloned() else {
        return;
    };
    let active = env.core.config.active_preset == id;

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, CARD_H), Sense::click());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 10.0, pal.card);
    let stroke = if active {
        Stroke::new(2.0_f32, pal.accent)
    } else if resp.hovered() {
        Stroke::new(1.0_f32, pal.accent.gamma_multiply(0.7))
    } else {
        Stroke::new(1.0_f32, pal.stroke)
    };
    p.rect_stroke(rect, 10.0, stroke);

    // thumbnail on a neutral dark tile
    let tile = Rect::from_min_size(
        rect.min + Vec2::splat(8.0),
        Vec2::new(rect.width() - 16.0, THUMB),
    );
    p.rect_filled(tile, 6.0, Color32::from_rgb(0x2A, 0x2F, 0x3A));
    if let Some(tid) = thumbnail(ui, env, id) {
        if let Some((_, tex)) = env.st.thumbs.get(id) {
            let ppp = ui.ctx().pixels_per_point();
            let size = tex.size_vec2() / ppp;
            let min = Pos2::new(
                ((tile.center().x - size.x / 2.0) * ppp).round() / ppp,
                ((tile.center().y - size.y / 2.0) * ppp).round() / ppp,
            );
            p.with_clip_rect(tile).image(
                tid,
                Rect::from_min_size(min, size),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }

    // name + badges
    p.text(
        Pos2::new(rect.left() + 10.0, tile.bottom() + 14.0),
        Align2::LEFT_CENTER,
        &preset.name,
        egui::FontId::proportional(14.0),
        pal.text,
    );
    let mut badge = String::new();
    if preset.builtin {
        badge.push_str(tr(l, "встроенный", "built-in"));
    }
    if let Some(h) = &preset.hotkey {
        if !badge.is_empty() {
            badge.push_str(" | ");
        }
        badge.push_str(h);
    }
    if active {
        p.text(
            Pos2::new(rect.right() - 10.0, tile.bottom() + 14.0),
            Align2::RIGHT_CENTER,
            tr(l, "активен", "active"),
            egui::FontId::proportional(12.0),
            pal.accent,
        );
    }
    if !badge.is_empty() {
        p.text(
            Pos2::new(rect.left() + 10.0, tile.bottom() + 32.0),
            Align2::LEFT_CENTER,
            badge,
            egui::FontId::proportional(11.5),
            pal.muted,
        );
    }

    if resp.clicked() {
        env.core.config.set_active(id);
        env.changed = true;
    }
    if resp.double_clicked() {
        env.st.page = Page::Editor;
    }

    resp.context_menu(|ui| {
        if ui.button(tr(l, "Редактировать", "Edit")).clicked() {
            env.core.config.set_active(id);
            env.st.page = Page::Editor;
            env.changed = true;
            ui.close_menu();
        }
        if ui.button(tr(l, "Переименовать", "Rename")).clicked() {
            env.st.rename = Some((id.to_string(), preset.name.clone()));
            ui.close_menu();
        }
        if ui.button(tr(l, "Дублировать", "Duplicate")).clicked() {
            if let Some(nid) = env
                .core
                .config
                .duplicate_preset(id, tr(l, "(копия)", "(copy)"))
            {
                env.core.config.set_active(&nid);
                env.changed = true;
            }
            ui.close_menu();
        }
        if ui
            .button(tr(l, "Горячая клавиша...", "Hotkey..."))
            .clicked()
        {
            env.core.config.set_active(id);
            env.st.page = Page::Hotkeys;
            env.changed = true;
            ui.close_menu();
        }
        if ui
            .button(tr(l, "Копировать как текст", "Copy as text"))
            .clicked()
        {
            env.actions.push(Action::CopyPreset(id.to_string()));
            ui.close_menu();
        }
        if ui
            .button(tr(l, "Экспорт в файл...", "Export to file..."))
            .clicked()
        {
            env.actions.push(Action::ExportPreset(id.to_string()));
            ui.close_menu();
        }
        ui.separator();
        let del = ui.add_enabled(
            !preset.builtin,
            egui::Button::new(RichText::new(tr(l, "Удалить", "Delete")).color(pal.danger)),
        );
        if del.clicked() {
            if let Some((removed, index)) = env.core.config.delete_preset(id) {
                env.st.thumbs.remove(id);
                env.st.toasts.retain(|t| !t.undo);
                env.st.toasts.push(UiToast {
                    kind: ToastKind::Info,
                    text: format!("{}: {}", tr(l, "Удалён", "Deleted"), removed.name),
                    created: Instant::now(),
                    undo: true,
                });
                env.st.undo = Some(Undo {
                    preset: removed,
                    index,
                    at: Instant::now(),
                });
                env.changed = true;
            }
            ui.close_menu();
        }
        if preset.builtin {
            del.on_disabled_hover_text(tr(
                l,
                "Встроенные пресеты удалить нельзя",
                "Built-in presets cannot be deleted",
            ));
        }
    });
}

/// Rename dialog (opened from several places).
pub fn dialogs(ctx: &egui::Context, env: &mut Env) {
    let l = env.lang;
    let pal = env.pal;
    if let Some(u) = &env.st.undo {
        if u.at.elapsed().as_secs() > 6 {
            env.st.undo = None;
        }
    }
    let Some((id, mut buf)) = env.st.rename.clone() else {
        return;
    };
    let mut close = false;
    let mut apply = false;
    egui::Window::new(tr(l, "Название пресета", "Preset name"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut buf).desired_width(260.0));
            if !r.has_focus() && !r.lost_focus() {
                r.request_focus();
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                apply = true;
            }
            ui.horizontal(|ui| {
                if primary_button(ui, &pal, "OK").clicked() {
                    apply = true;
                }
                if ui.button(tr(l, "Отмена", "Cancel")).clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    close = true;
                }
            });
        });
    if apply {
        env.core.config.rename_preset(&id, &buf);
        env.changed = true;
        close = true;
    }
    if close {
        env.st.rename = None;
    } else {
        env.st.rename = Some((id, buf));
    }
}
