//! Reusable controls: slider rows, segmented buttons, colour picker, cards, toggle switch.

use std::ops::RangeInclusive;

use eframe::egui::{
    self, color_picker, Align, Align2, Color32, FontId, Layout, Pos2, Rect, Response, RichText,
    Sense, Stroke, Ui, Vec2,
};

use super::theme::Palette;
use super::tr;
use crate::config::{Lang, Rgba};

pub const LABEL_W: f32 = 118.0;

pub fn card<R>(ui: &mut Ui, pal: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::none()
        .fill(pal.card)
        .stroke(Stroke::new(1.0_f32, pal.stroke))
        .rounding(10.0)
        .inner_margin(egui::Margin::same(12.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn heading(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).heading().strong());
}

pub fn muted(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.label(RichText::new(text).small().color(pal.muted));
}

pub fn primary_button(ui: &mut Ui, pal: &Palette, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(pal.on_accent))
            .fill(pal.accent)
            .stroke(Stroke::NONE)
            .rounding(6.0),
    )
}

pub fn danger_button(ui: &mut Ui, pal: &Palette, text: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(text).color(pal.danger)).rounding(6.0))
}

/// Pill-style toggle switch. Returns true when the value changed.
pub fn switch(ui: &mut Ui, pal: &Palette, on: &mut bool) -> Response {
    let size = Vec2::new(38.0, 20.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let bg = lerp_color(pal.stroke, pal.accent, t);
    ui.painter().rect_filled(rect, 10.0, bg);
    let knob_x = egui::lerp((rect.left() + 10.0)..=(rect.right() - 10.0), t);
    ui.painter()
        .circle_filled(Pos2::new(knob_x, rect.center().y), 7.0, Color32::WHITE);
    if resp.has_focus() {
        ui.painter()
            .rect_stroke(rect.expand(2.0), 11.0, Stroke::new(1.5_f32, pal.accent));
    }
    resp
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Label on the left, switch on the right.
pub fn bool_row(ui: &mut Ui, pal: &Palette, label: &str, on: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized(
            [LABEL_W + 40.0, 22.0],
            egui::Label::new(label).halign(Align::LEFT),
        );
        changed = switch(ui, pal, on).changed();
    });
    changed
}

/// `label | slider | numeric field`. Double click resets the slider.
pub fn slider_row(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    range: RangeInclusive<f32>,
    step: f32,
    default: f32,
    unit: &str,
    decimals: usize,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized([LABEL_W, 22.0], egui::Label::new(label).halign(Align::LEFT));
        let avail = ui.available_width();
        ui.spacing_mut().slider_width = (avail - 84.0).max(70.0);
        let before = *value;
        let s = ui.add(
            egui::Slider::new(value, range.clone())
                .show_value(false)
                .step_by(step as f64)
                .smart_aim(false),
        );
        let slider_widgets = &ui.visuals().widgets;
        let outline = if s.dragged() {
            slider_widgets.active.bg_stroke
        } else if s.hovered() || s.has_focus() {
            slider_widgets.hovered.bg_stroke
        } else {
            slider_widgets.inactive.bg_stroke
        };
        ui.painter().rect_stroke(s.rect, 4.0, outline);
        ui.add(
            egui::DragValue::new(value)
                .speed(step as f64)
                .range(range.clone())
                .fixed_decimals(decimals)
                .suffix(unit),
        );
        if s.double_clicked() {
            *value = default;
        }
        if (*value - before).abs() > f32::EPSILON {
            changed = true;
        }
    });
    changed
}

/// Row of mutually exclusive buttons.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    pal: &Palette,
    items: &[(T, String)],
    current: &mut T,
) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (value, text) in items {
            let selected = *current == *value;
            let btn = egui::Button::new(RichText::new(text).color(if selected {
                pal.on_accent
            } else {
                pal.text
            }))
            .fill(if selected { pal.accent } else { pal.card })
            .rounding(6.0);
            if ui.add(btn).clicked() && !selected {
                *current = *value;
                changed = true;
            }
        }
    });
    changed
}

fn to_c32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], c.0[3])
}

pub fn checker(painter: &egui::Painter, rect: Rect, cell: f32, a: Color32, b: Color32) {
    painter.rect_filled(rect, 0.0, a);
    let cols = (rect.width() / cell).ceil() as i32;
    let rows = (rect.height() / cell).ceil() as i32;
    for y in 0..rows {
        for x in 0..cols {
            if (x + y) % 2 == 1 {
                let min = rect.min + Vec2::new(x as f32 * cell, y as f32 * cell);
                let r = Rect::from_min_size(min, Vec2::splat(cell)).intersect(rect);
                painter.rect_filled(r, 0.0, b);
            }
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct ColorOut {
    pub changed: bool,
    /// The edit is finished (pointer released / Enter): store it in "recent colours".
    pub commit: bool,
    pub eyedropper: bool,
}

/// Swatch + hex label; clicking opens the picker popup with hex field, recent colours, eyedropper.
pub fn color_row(
    ui: &mut Ui,
    pal: &Palette,
    lang: Lang,
    id: &str,
    label: &str,
    color: &mut Rgba,
    recent: &[Rgba],
) -> ColorOut {
    let mut out = ColorOut::default();
    ui.horizontal(|ui| {
        ui.add_sized([LABEL_W, 22.0], egui::Label::new(label).halign(Align::LEFT));
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(52.0, 22.0), Sense::click());
        let painter = ui.painter_at(rect);
        checker(
            &painter,
            rect,
            6.0,
            Color32::from_gray(200),
            Color32::from_gray(140),
        );
        painter.rect_filled(rect, 0.0, to_c32(*color));
        ui.painter().rect_stroke(
            rect,
            5.0,
            Stroke::new(
                1.0_f32,
                if resp.hovered() {
                    pal.accent
                } else {
                    pal.stroke
                },
            ),
        );
        ui.label(RichText::new(color.to_hex()).monospace().color(pal.muted));

        let popup_id = ui.make_persistent_id(("color_popup", id));
        if resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup_id));
        }
        egui::popup_below_widget(
            ui,
            popup_id,
            &resp,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(250.0);
                let mut c32 = to_c32(*color);
                let before = *color;
                if color_picker::color_picker_color32(ui, &mut c32, color_picker::Alpha::OnlyBlend)
                {
                    let [r, g, b, a] = c32.to_srgba_unmultiplied();
                    *color = Rgba([r, g, b, a]);
                    ui.data_mut(|d| d.insert_temp(popup_id.with("dirty"), true));
                }

                // hex field
                ui.horizontal(|ui| {
                    ui.label("HEX");
                    let hex_id = ui.id().with("hex");
                    let mut text: String = ui
                        .data(|d| d.get_temp(hex_id))
                        .unwrap_or_else(|| color.to_hex());
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut text)
                            .desired_width(96.0)
                            .font(FontId::monospace(13.0)),
                    );
                    if r.changed() {
                        if let Some(n) = Rgba::from_hex(&text) {
                            *color = n;
                        }
                        ui.data_mut(|d| d.insert_temp(hex_id, text.clone()));
                    } else if !r.has_focus() {
                        ui.data_mut(|d| d.remove::<String>(hex_id));
                    }
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        out.commit = true;
                    }
                    if ui.button(tr(lang, "Пипетка", "Eyedropper")).clicked() {
                        out.eyedropper = true;
                        ui.memory_mut(|m| m.close_popup());
                    }
                });

                if !recent.is_empty() {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(tr(lang, "Недавние", "Recent"))
                            .small()
                            .color(pal.muted),
                    );
                    ui.horizontal_wrapped(|ui| {
                        for c in recent {
                            let (r, resp) =
                                ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                            let p = ui.painter_at(r);
                            checker(&p, r, 5.0, Color32::from_gray(200), Color32::from_gray(140));
                            p.rect_filled(r, 0.0, to_c32(*c));
                            ui.painter().rect_stroke(
                                r,
                                4.0,
                                Stroke::new(
                                    1.0_f32,
                                    if resp.hovered() {
                                        pal.accent
                                    } else {
                                        pal.stroke
                                    },
                                ),
                            );
                            if resp.on_hover_text(c.to_hex()).clicked() {
                                *color = *c;
                                out.commit = true;
                            }
                        }
                    });
                }
                if *color != before {
                    out.changed = true;
                }
            },
        );
        // a drag on the picker is finished once the pointer is up again and nothing changed
        let dirty_id = popup_id.with("dirty");
        let dirty: bool = ui.data(|d| d.get_temp(dirty_id)).unwrap_or(false);
        if dirty && !out.changed && !ui.input(|i| i.pointer.any_down()) {
            out.commit = true;
            ui.data_mut(|d| d.insert_temp(dirty_id, false));
        }
    });
    out
}

/// A small coloured dot + text, used by the status bar.
pub fn status_dot(ui: &mut Ui, color: Color32, text: &str, pal: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
    ui.label(RichText::new(text).small().color(pal.muted));
}

pub fn nav_item(ui: &mut Ui, pal: &Palette, label: &str, selected: bool) -> bool {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
    let hovered = resp.hovered() || resp.has_focus();
    if selected {
        ui.painter()
            .rect_filled(rect, 8.0, pal.accent.gamma_multiply(0.22));
        ui.painter().rect_filled(
            Rect::from_min_size(
                rect.min + Vec2::new(0.0, 7.0),
                Vec2::new(3.0, rect.height() - 14.0),
            ),
            2.0,
            pal.accent,
        );
    } else if hovered {
        ui.painter().rect_filled(rect, 8.0, pal.card);
    }
    ui.painter().text(
        rect.left_center() + Vec2::new(14.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.5),
        if selected { pal.text } else { pal.muted },
    );
    resp.clicked()
}

/// Right-aligned helper for button rows.
pub fn right_aligned<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.with_layout(Layout::right_to_left(Align::Center), add)
        .inner
}
