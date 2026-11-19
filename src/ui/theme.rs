//! Dark / light palettes and the egui style derived from them.

use eframe::egui::{self, Color32, FontFamily, FontId, Rounding, Stroke, TextStyle, Visuals};

use crate::config::ThemeMode;

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub stroke: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub danger: Color32,
    pub success: Color32,
    pub warn: Color32,
}

fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            dark,
            bg: hex(0x14161A),
            panel: hex(0x1A1D23),
            card: hex(0x222630),
            stroke: hex(0x303644),
            text: hex(0xE6E8EC),
            muted: hex(0x9AA1AE),
            accent: hex(0x4C8DFF),
            on_accent: hex(0xFFFFFF),
            danger: hex(0xFF6B6B),
            success: hex(0x3DDC84),
            warn: hex(0xFFB020),
        }
    } else {
        Palette {
            dark,
            bg: hex(0xF1F3F6),
            panel: hex(0xFFFFFF),
            card: hex(0xFFFFFF),
            stroke: hex(0xD5D9E0),
            text: hex(0x1B1E24),
            muted: hex(0x5F6673),
            accent: hex(0x2F6FE0),
            on_accent: hex(0xFFFFFF),
            danger: hex(0xC93636),
            success: hex(0x1E9E5A),
            warn: hex(0xB36B00),
        }
    }
}

pub fn is_dark(ctx: &egui::Context, mode: ThemeMode) -> bool {
    match mode {
        ThemeMode::Dark => true,
        ThemeMode::Light => false,
        ThemeMode::System => ctx
            .input(|i| i.raw.system_theme)
            .map(|t| t == egui::Theme::Dark)
            .unwrap_or(true),
    }
}

pub fn apply(ctx: &egui::Context, pal: &Palette, reduce_motion: bool) {
    let mut v = if pal.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.panel_fill = pal.panel;
    v.window_fill = pal.panel;
    v.extreme_bg_color = pal.bg;
    v.faint_bg_color = pal.card;
    v.window_stroke = Stroke::new(1.0_f32, pal.stroke);
    v.window_rounding = Rounding::same(10.0);
    v.menu_rounding = Rounding::same(8.0);
    v.override_text_color = Some(pal.text);
    v.hyperlink_color = pal.accent;
    v.selection.bg_fill = pal.accent.gamma_multiply(0.45);
    v.selection.stroke = Stroke::new(1.0_f32, pal.accent);

    let r = Rounding::same(6.0);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = pal.card;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, pal.stroke);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, pal.text);
    w.noninteractive.rounding = r;
    w.inactive.bg_fill = pal.card;
    w.inactive.weak_bg_fill = pal.card;
    w.inactive.bg_stroke = Stroke::new(1.0_f32, pal.stroke);
    w.inactive.fg_stroke = Stroke::new(1.0_f32, pal.text);
    w.inactive.rounding = r;
    w.hovered.bg_fill = pal.card.gamma_multiply(if pal.dark { 1.25 } else { 0.96 });
    w.hovered.weak_bg_fill = w.hovered.bg_fill;
    w.hovered.bg_stroke = Stroke::new(1.0_f32, pal.accent);
    w.hovered.fg_stroke = Stroke::new(1.5_f32, pal.text);
    w.hovered.rounding = r;
    w.active.bg_fill = pal.accent.gamma_multiply(0.6);
    w.active.weak_bg_fill = w.active.bg_fill;
    w.active.bg_stroke = Stroke::new(1.0_f32, pal.accent);
    w.active.fg_stroke = Stroke::new(1.5_f32, pal.text);
    w.active.rounding = r;
    w.open.bg_fill = pal.card;
    w.open.rounding = r;
    ctx.set_visuals(v);

    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 7.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
        s.spacing.interact_size.y = 24.0;
        s.spacing.slider_width = 150.0;
        s.animation_time = if reduce_motion { 0.0 } else { 0.12 };
        s.text_styles = [
            (
                TextStyle::Small,
                FontId::new(12.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(14.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Heading,
                FontId::new(19.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(13.0, FontFamily::Monospace),
            ),
        ]
        .into();
    });
}
