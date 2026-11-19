//! "Crosshair" page: live preview on the left, parameters on the right.

use eframe::egui::{
    self, Align2, Color32, ColorImage, Pos2, Rect, RichText, Sense, Stroke, TextureOptions, Ui,
    Vec2,
};

use super::widgets::{self, bool_row, card, color_row, primary_button, segmented, slider_row};
use super::{tr, Action, ColorSlot, Env, Eye, PreviewBg};
use crate::config::{Crosshair, ImageSource, Rgba, Shape};
use crate::render;

const PREVIEW: f32 = 292.0;

pub fn show(ui: &mut Ui, env: &mut Env) {
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(PREVIEW);
            preview_panel(ui, env);
        });
        ui.add_space(8.0);
        ui.vertical(|ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    params(ui, env);
                });
        });
    });
}

fn preview_panel(ui: &mut Ui, env: &mut Env) {
    let l = env.lang;
    let pal = env.pal;
    let ctx = ui.ctx().clone();

    // --- render (only when something changed)
    let c = env.core.config.active().crosshair.clone();
    let zoom = env.st.preview_zoom;
    let paths = env.core.paths.clone();
    let bitmap = c
        .image
        .as_ref()
        .filter(|_| c.shape == Shape::Image)
        .and_then(|i| env.core.images.get(&paths, &i.file));
    let key = (c.clone(), zoom.to_bits());
    if env.st.preview_key.as_ref() != Some(&key) || env.st.preview_tex.is_none() {
        let pm = render::render(&c, zoom, bitmap.as_deref());
        let img = ColorImage::from_rgba_premultiplied(
            [pm.width() as usize, pm.height() as usize],
            pm.data(),
        );
        match &mut env.st.preview_tex {
            Some(t) => t.set(img, TextureOptions::NEAREST),
            None => {
                env.st.preview_tex = Some(ctx.load_texture("preview", img, TextureOptions::NEAREST))
            }
        }
        env.st.preview_key = Some(key);
    }

    // --- background + image
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(PREVIEW), Sense::hover());
    let painter = ui.painter_at(rect);
    paint_background(&painter, rect, env.st.preview_bg);
    painter.rect_stroke(rect, 8.0, Stroke::new(1.0_f32, pal.stroke));
    if let Some(tex) = &env.st.preview_tex {
        let ppp = ctx.pixels_per_point();
        let size = tex.size_vec2() / ppp;
        let snap = |v: f32| (v * ppp).round() / ppp;
        let min = Pos2::new(
            snap(rect.center().x - size.x / 2.0),
            snap(rect.center().y - size.y / 2.0),
        );
        painter.image(
            tex.id(),
            Rect::from_min_size(min, size),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    if c.shape == Shape::Image && bitmap.is_none() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            tr(l, "Выберите PNG-файл", "Choose a PNG file"),
            egui::FontId::proportional(14.0),
            Color32::from_gray(210),
        );
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(tr(l, "Фон", "Background"))
                .small()
                .color(pal.muted),
        );
        let items = vec![
            (PreviewBg::Game, tr(l, "Игра", "Game").to_string()),
            (PreviewBg::Light, tr(l, "Светлый", "Light").to_string()),
            (PreviewBg::Dark, tr(l, "Тёмный", "Dark").to_string()),
            (PreviewBg::Checker, tr(l, "Шахматка", "Checker").to_string()),
        ];
        let mut bg = env.st.preview_bg;
        if segmented(ui, &pal, &items, &mut bg) {
            env.st.preview_bg = bg;
        }
    });
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(tr(l, "Масштаб", "Zoom"))
                .small()
                .color(pal.muted),
        );
        for z in [1.0f32, 2.0, 4.0, 8.0] {
            let sel = (env.st.preview_zoom - z).abs() < 0.01;
            if ui.selectable_label(sel, format!("{z}x")).clicked() {
                env.st.preview_zoom = z;
            }
        }
    });
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        if ui.button(tr(l, "Сбросить", "Reset")).clicked() {
            let id = env.core.config.active_preset.clone();
            env.core.config.reset_preset(&id);
            env.changed = true;
        }
        if primary_button(ui, &pal, tr(l, "Сохранить как пресет", "Save as preset")).clicked()
        {
            let src = env.core.config.active_preset.clone();
            if let Some(id) = env
                .core
                .config
                .duplicate_preset(&src, tr(l, "(копия)", "(copy)"))
            {
                env.core.config.set_active(&id);
                env.st.rename = Some((
                    id.clone(),
                    env.core
                        .config
                        .preset(&id)
                        .map(|p| p.name.clone())
                        .unwrap_or_default(),
                ));
                env.changed = true;
            }
        }
    });
    ui.add_space(4.0);
    widgets::muted(
        ui,
        &pal,
        &format!(
            "{}: {}",
            tr(l, "Редактируется", "Editing"),
            env.core.config.active().name
        ),
    );
}

fn paint_background(p: &egui::Painter, rect: Rect, bg: PreviewBg) {
    match bg {
        PreviewBg::Light => {
            p.rect_filled(rect, 8.0, Color32::from_rgb(0xE6, 0xE8, 0xEC));
        }
        PreviewBg::Dark => {
            p.rect_filled(rect, 8.0, Color32::from_rgb(0x12, 0x14, 0x18));
        }
        PreviewBg::Checker => {
            widgets::checker(
                p,
                rect,
                14.0,
                Color32::from_gray(90),
                Color32::from_gray(130),
            );
        }
        PreviewBg::Game => {
            // sky gradient, a sun and a few silhouettes: a stand-in for a game screenshot
            let top = Color32::from_rgb(0x2C, 0x4A, 0x6B);
            let bottom = Color32::from_rgb(0xC9, 0x8A, 0x5B);
            let mut mesh = egui::Mesh::default();
            mesh.colored_vertex(rect.left_top(), top);
            mesh.colored_vertex(rect.right_top(), top);
            mesh.colored_vertex(rect.right_bottom(), bottom);
            mesh.colored_vertex(rect.left_bottom(), bottom);
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            p.add(egui::Shape::mesh(mesh));
            p.circle_filled(
                rect.left_top() + Vec2::new(PREVIEW * 0.72, PREVIEW * 0.55),
                26.0,
                Color32::from_rgb(0xFF, 0xE2, 0xA8),
            );
            let ground = Color32::from_rgb(0x1A, 0x1F, 0x26);
            let w = rect.width();
            let h = rect.height();
            for (x, bw, bh) in [
                (0.04, 0.16, 0.30),
                (0.22, 0.12, 0.20),
                (0.40, 0.10, 0.34),
                (0.58, 0.18, 0.16),
                (0.80, 0.14, 0.28),
            ] {
                let r = Rect::from_min_size(
                    Pos2::new(rect.left() + w * x, rect.bottom() - h * bh),
                    Vec2::new(w * bw, h * bh),
                );
                p.rect_filled(r, 0.0, ground);
            }
        }
    }
}

fn section(ui: &mut Ui, text: &str, pal: &super::theme::Palette) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).strong().color(pal.muted).small());
}

fn params(ui: &mut Ui, env: &mut Env) {
    let l = env.lang;
    let pal = env.pal;
    let recent = env.core.config.general.recent_colors.clone();
    let mut eye_request: Option<ColorSlot> = None;
    let mut commit: Option<Rgba> = None;
    let mut pick_png = false;
    let mut changed = false;

    {
        let c: &mut Crosshair = &mut env.core.config.active_mut().crosshair;

        card(ui, &pal, |ui| {
            section(ui, tr(l, "ФОРМА", "SHAPE"), &pal);
            let items: Vec<(Shape, String)> = Shape::ALL
                .iter()
                .map(|s| (*s, s.label(l).to_string()))
                .collect();
            let mut shape = c.shape;
            if segmented(ui, &pal, &items, &mut shape) {
                c.shape = shape;
                changed = true;
            }
            ui.add_space(6.0);

            if c.shape != Shape::Image {
                let out = color_row(
                    ui,
                    &pal,
                    l,
                    "main",
                    tr(l, "Цвет", "Colour"),
                    &mut c.color,
                    &recent,
                );
                changed |= out.changed;
                if out.commit {
                    commit = Some(c.color);
                }
                if out.eyedropper {
                    eye_request = Some(ColorSlot::Main);
                }
            }
            let mut pct = c.opacity * 100.0;
            if slider_row(
                ui,
                tr(l, "Непрозрачность", "Opacity"),
                &mut pct,
                0.0..=100.0,
                1.0,
                100.0,
                "%",
                0,
            ) {
                c.opacity = pct / 100.0;
                changed = true;
            }

            let shape = c.shape;
            let has_arms = matches!(shape, Shape::Cross | Shape::T);
            let has_ring = matches!(shape, Shape::Circle | Shape::CircleDot | Shape::Square);
            if has_arms || has_ring {
                changed |= slider_row(
                    ui,
                    tr(l, "Толщина", "Thickness"),
                    &mut c.thickness,
                    0.5..=24.0,
                    0.5,
                    2.0,
                    " px",
                    1,
                );
            }
            if has_arms {
                changed |= slider_row(
                    ui,
                    tr(l, "Длина", "Length"),
                    &mut c.length,
                    0.0..=80.0,
                    1.0,
                    8.0,
                    " px",
                    0,
                );
                changed |= slider_row(
                    ui,
                    tr(l, "Зазор", "Gap"),
                    &mut c.gap,
                    0.0..=80.0,
                    1.0,
                    4.0,
                    " px",
                    0,
                );
            }
            if has_ring {
                changed |= slider_row(
                    ui,
                    tr(l, "Радиус", "Radius"),
                    &mut c.radius,
                    2.0..=120.0,
                    1.0,
                    12.0,
                    " px",
                    0,
                );
            }
            if matches!(shape, Shape::Cross | Shape::T | Shape::Square) {
                changed |= bool_row(
                    ui,
                    &pal,
                    tr(l, "Точка в центре", "Centre dot"),
                    &mut c.center_dot,
                );
            }
            if matches!(shape, Shape::Dot | Shape::CircleDot)
                || (c.center_dot && matches!(shape, Shape::Cross | Shape::T | Shape::Square))
            {
                changed |= slider_row(
                    ui,
                    tr(l, "Размер точки", "Dot size"),
                    &mut c.dot_size,
                    1.0..=40.0,
                    1.0,
                    4.0,
                    " px",
                    0,
                );
            }
            if has_arms {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [widgets::LABEL_W, 22.0],
                        egui::Label::new(tr(l, "Сегменты", "Segments")),
                    );
                    changed |= ui
                        .checkbox(&mut c.segments.top, tr(l, "верх", "top"))
                        .changed();
                    changed |= ui
                        .checkbox(&mut c.segments.bottom, tr(l, "низ", "bottom"))
                        .changed();
                    changed |= ui
                        .checkbox(&mut c.segments.left, tr(l, "лево", "left"))
                        .changed();
                    changed |= ui
                        .checkbox(&mut c.segments.right, tr(l, "право", "right"))
                        .changed();
                });
            }

            if shape == Shape::Image {
                ui.horizontal(|ui| {
                    if primary_button(ui, &pal, tr(l, "Выбрать PNG...", "Choose PNG...")).clicked()
                    {
                        pick_png = true;
                    }
                    let name = c.image.as_ref().map(|i| i.file.clone()).unwrap_or_default();
                    if !name.is_empty() {
                        widgets::muted(ui, &pal, &name);
                    }
                });
                let img = c.image.get_or_insert_with(ImageSource::default);
                changed |= slider_row(
                    ui,
                    tr(l, "Масштаб картинки", "Image scale"),
                    &mut img.scale,
                    0.05..=4.0,
                    0.05,
                    1.0,
                    "x",
                    2,
                );
                let mut tinted = img.tint.is_some();
                if bool_row(ui, &pal, tr(l, "Перекрасить", "Tint"), &mut tinted) {
                    img.tint = if tinted {
                        Some(Rgba::new(255, 255, 255, 255))
                    } else {
                        None
                    };
                    changed = true;
                }
                if let Some(t) = img.tint.as_mut() {
                    let out = color_row(
                        ui,
                        &pal,
                        l,
                        "tint",
                        tr(l, "Цвет заливки", "Tint colour"),
                        t,
                        &recent,
                    );
                    changed |= out.changed;
                    if out.commit {
                        commit = Some(*t);
                    }
                    if out.eyedropper {
                        eye_request = Some(ColorSlot::Tint);
                    }
                }
            }
        });

        ui.add_space(8.0);
        card(ui, &pal, |ui| {
            egui::CollapsingHeader::new(RichText::new(tr(l, "Обводка", "Outline")).strong())
                .id_salt("outline")
                .default_open(c.outline.enabled)
                .show(ui, |ui| {
                    changed |= bool_row(
                        ui,
                        &pal,
                        tr(l, "Включить", "Enabled"),
                        &mut c.outline.enabled,
                    );
                    if c.outline.enabled {
                        let out = color_row(
                            ui,
                            &pal,
                            l,
                            "outline",
                            tr(l, "Цвет", "Colour"),
                            &mut c.outline.color,
                            &recent,
                        );
                        changed |= out.changed;
                        if out.commit {
                            commit = Some(c.outline.color);
                        }
                        if out.eyedropper {
                            eye_request = Some(ColorSlot::Outline);
                        }
                        changed |= slider_row(
                            ui,
                            tr(l, "Толщина", "Width"),
                            &mut c.outline.thickness,
                            0.5..=8.0,
                            0.5,
                            1.0,
                            " px",
                            1,
                        );
                        let mut pct = c.outline.opacity * 100.0;
                        if slider_row(
                            ui,
                            tr(l, "Непрозрачность", "Opacity"),
                            &mut pct,
                            0.0..=100.0,
                            1.0,
                            100.0,
                            "%",
                            0,
                        ) {
                            c.outline.opacity = pct / 100.0;
                            changed = true;
                        }
                    }
                });
        });

        ui.add_space(8.0);
        card(ui, &pal, |ui| {
            egui::CollapsingHeader::new(
                RichText::new(tr(l, "Положение и вид", "Position and look")).strong(),
            )
            .id_salt("placement")
            .default_open(true)
            .show(ui, |ui| {
                changed |= slider_row(
                    ui,
                    tr(l, "Масштаб", "Scale"),
                    &mut c.scale,
                    0.25..=4.0,
                    0.05,
                    1.0,
                    "x",
                    2,
                );
                changed |= slider_row(
                    ui,
                    tr(l, "Поворот", "Rotation"),
                    &mut c.rotation,
                    -180.0..=180.0,
                    1.0,
                    0.0,
                    "°",
                    0,
                );
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [widgets::LABEL_W, 22.0],
                        egui::Label::new(tr(l, "Смещение", "Offset")),
                    );
                    ui.label("X");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut c.offset.0)
                                .range(-2000..=2000)
                                .suffix(" px"),
                        )
                        .changed();
                    ui.label("Y");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut c.offset.1)
                                .range(-2000..=2000)
                                .suffix(" px"),
                        )
                        .changed();
                    if ui.button(tr(l, "Центр", "Centre")).clicked() {
                        c.offset = (0, 0);
                        changed = true;
                    }
                });
                // nudge pad
                ui.horizontal(|ui| {
                    ui.add_space(widgets::LABEL_W + 8.0);
                    for (txt, dx, dy) in [("<", -1, 0), (">", 1, 0), ("^", 0, -1), ("v", 0, 1)] {
                        if ui.small_button(txt).clicked() {
                            c.offset.0 += dx;
                            c.offset.1 += dy;
                            changed = true;
                        }
                    }
                });
                changed |= bool_row(
                    ui,
                    &pal,
                    tr(l, "Чёткие пиксели", "Pixel-perfect"),
                    &mut c.pixel_perfect,
                );
                if c.pixel_perfect && c.rotation.rem_euclid(360.0) != 0.0 {
                    widgets::muted(
                        ui,
                        &pal,
                        tr(
                            l,
                            "При повороте сглаживание включается автоматически",
                            "Anti-aliasing is used while rotated",
                        ),
                    );
                }
            });
        });
        ui.add_space(8.0);
        if ui
            .button(tr(
                l,
                "Показать рамку монитора на 3 секунды",
                "Show monitor frame for 3 seconds",
            ))
            .clicked()
        {
            env.ctl.send(crate::overlay::Command::FlashFrame);
        }
    }

    if let Some(col) = commit {
        env.core.config.push_recent_color(col);
        changed = true;
    }
    if let Some(slot) = eye_request {
        env.st.eye = Some(Eye {
            slot,
            seen_up: false,
        });
    }
    if pick_png {
        env.actions.push(Action::PickPng);
    }
    if changed {
        env.changed = true;
    }
}
