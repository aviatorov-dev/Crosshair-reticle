//! Pure, deterministic crosshair renderer: `Crosshair` -> premultiplied RGBA pixmap.
//! No Win32 or UI dependencies, so it is fully unit-testable.
//!
//! Approach: the geometry is drawn twice as opaque coverage masks (the shape itself and the
//! shape grown by the outline width). The masks are then coloured and composited manually, which
//! keeps overlapping segments from blending twice and lets the outline stay *outside* the fill.

mod image_crosshair;
mod shapes;

use image::RgbaImage;
use tiny_skia::Pixmap;

use crate::config::{Crosshair, Shape};

pub use image_crosshair::premultiply_image;

pub const MIN_CANVAS: u32 = 16;
pub const MAX_CANVAS: u32 = 2048;

/// Pixel values after applying `scale`, already snapped in pixel-perfect mode.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Params {
    pub t: f32,
    pub len: f32,
    pub gap: f32,
    pub dot: f32,
    pub radius: f32,
    pub outline: f32,
}

/// Pixel-perfect mode only works for unrotated shapes.
pub(crate) fn is_crisp(c: &Crosshair) -> bool {
    c.pixel_perfect && c.rotation.rem_euclid(360.0) == 0.0
}

pub(crate) fn params(c: &Crosshair, scale: f32) -> Params {
    let s = (c.scale * scale).max(0.01);
    let mut p = Params {
        t: (c.thickness * s).max(0.5),
        len: c.length * s,
        gap: c.gap * s,
        dot: c.dot_size * s,
        radius: c.radius * s,
        outline: if c.outline.enabled {
            c.outline.thickness * s
        } else {
            0.0
        },
    };
    if is_crisp(c) {
        p.t = p.t.round().max(1.0);
        p.len = p.len.round();
        p.gap = p.gap.round();
        p.dot = if p.dot >= 0.5 {
            p.dot.round().max(1.0)
        } else {
            0.0
        };
        p.radius = p.radius.round().max(1.0);
        p.outline = if c.outline.enabled && p.outline > 0.0 {
            p.outline.round().max(1.0)
        } else {
            0.0
        };
    }
    p
}

/// Farthest distance from the centre that the shape (with outline) can reach.
fn half_extent(c: &Crosshair, scale: f32, bitmap: Option<&RgbaImage>) -> f32 {
    let p = params(c, scale);
    let rotated = c.rotation.rem_euclid(90.0) != 0.0;
    let arm = |reach: f32| {
        if rotated {
            reach.hypot(p.t / 2.0)
        } else {
            reach.max(p.t / 2.0)
        }
    };
    let base = match c.shape {
        Shape::Cross | Shape::T => {
            let mut e = arm(p.gap + p.len);
            if c.center_dot {
                e = e.max(p.dot / 2.0);
            }
            e
        }
        Shape::Dot => p.dot / 2.0,
        Shape::Circle | Shape::CircleDot => p.radius + p.t / 2.0,
        Shape::Square => {
            let h = p.radius + p.t / 2.0;
            if rotated {
                h * std::f32::consts::SQRT_2
            } else {
                h
            }
        }
        Shape::Image => {
            let (w, h) = bitmap
                .map(|b| (b.width() as f32, b.height() as f32))
                .unwrap_or((0.0, 0.0));
            let k = c.scale * scale * c.image.as_ref().map(|i| i.scale).unwrap_or(1.0);
            if rotated {
                w.hypot(h) / 2.0 * k
            } else {
                w.max(h) / 2.0 * k
            }
        }
    };
    base + p.outline
}

/// Side of the square canvas (always even, so the centre is a pixel boundary).
pub fn canvas_size(c: &Crosshair, scale: f32, bitmap: Option<&RgbaImage>) -> u32 {
    let e = half_extent(c, scale, bitmap);
    let side = (e * 2.0).ceil() as i64 + 4;
    let side = side.clamp(MIN_CANVAS as i64, MAX_CANVAS as i64) as u32;
    side + (side & 1)
}

/// Renders the crosshair. `scale` is an extra factor (DPI, preview zoom) on top of `c.scale`.
/// `bitmap` is only used for `Shape::Image`.
pub fn render(c: &Crosshair, scale: f32, bitmap: Option<&RgbaImage>) -> Pixmap {
    let n = canvas_size(c, scale, bitmap);
    if c.shape == Shape::Image {
        return image_crosshair::render_image(c, scale, bitmap, n);
    }
    let p = params(c, scale);
    let crisp = is_crisp(c);

    let mut fill = Pixmap::new(n, n).expect("canvas size is valid");
    shapes::draw_coverage(&mut fill, c, &p, 0.0, crisp);
    let mut grown = None;
    if c.outline.enabled && p.outline > 0.0 {
        let mut o = Pixmap::new(n, n).expect("canvas size is valid");
        shapes::draw_coverage(&mut o, c, &p, p.outline, crisp);
        grown = Some(o);
    }

    let fc = c.color.0;
    let fill_alpha = fc[3] as f32 / 255.0 * c.opacity;
    let oc = c.outline.color.0;
    let out_alpha = oc[3] as f32 / 255.0 * c.outline.opacity * c.opacity;

    let mut out = Pixmap::new(n, n).expect("canvas size is valid");
    let fpx = fill.pixels();
    let opx = grown.as_ref().map(|g| g.pixels());
    let dst = out.data_mut();
    for i in 0..(n * n) as usize {
        let f = fpx[i].alpha() as f32 / 255.0;
        let o = opx.map(|px| px[i].alpha() as f32 / 255.0).unwrap_or(0.0) * (1.0 - f);
        let fa = f * fill_alpha;
        let oa = o * out_alpha;
        let a = (fa + oa).min(1.0);
        if a <= 0.0 {
            continue;
        }
        let ch = |k: usize| {
            (fc[k] as f32 * fa + oc[k] as f32 * oa)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        let d = &mut dst[i * 4..i * 4 + 4];
        d[0] = ch(0).min((a * 255.0).round() as u8);
        d[1] = ch(1).min((a * 255.0).round() as u8);
        d[2] = ch(2).min((a * 255.0).round() as u8);
        d[3] = (a * 255.0).round() as u8;
    }
    out
}

#[cfg(test)]
mod tests;
