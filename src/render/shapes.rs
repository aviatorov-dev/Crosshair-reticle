//! Geometry of every shape, drawn as opaque black coverage into a pixmap.

use tiny_skia::{FillRule, Paint, Path, PathBuilder, Pixmap, Rect, Transform};

use super::Params;
use crate::config::{Crosshair, Shape};

fn push_rect(pb: &mut PathBuilder, l: f32, t: f32, r: f32, b: f32) -> bool {
    match Rect::from_ltrb(l, t, r, b) {
        Some(rc) => {
            pb.push_rect(rc);
            true
        }
        None => false,
    }
}

/// `grow` expands every edge outwards (used for the outline layer).
pub(super) fn draw_coverage(pm: &mut Pixmap, c: &Crosshair, p: &Params, grow: f32, crisp: bool) {
    let n = pm.width() as f32;
    let cx = n / 2.0;
    let cy = n / 2.0;

    let mut paint = Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = !crisp;

    let rot = c.rotation.rem_euclid(360.0);
    let tf = if rot != 0.0 {
        Transform::from_rotate_at(rot, cx, cy)
    } else {
        Transform::identity()
    };

    let mut fill = |path: Option<Path>, rule: FillRule| {
        if let Some(path) = path {
            pm.fill_path(&path, &paint, rule, tf, None);
        }
    };

    let t = p.t;
    // thickness split: low side / high side of the centre line
    let (lo_side, hi_side) = if crisp {
        let a = (t / 2.0).floor();
        (a, t - a)
    } else {
        (t / 2.0, t / 2.0)
    };
    let odd = if crisp { (t as i64 & 1) as f32 } else { 0.0 };

    match c.shape {
        Shape::Cross | Shape::T => {
            // empty zone around the centre: [zlo, zhi]
            let zlo = cx - p.gap;
            let zhi = cx + p.gap + odd;
            let bar_lo = cx - lo_side - grow;
            let bar_hi = cx + hi_side + grow;
            let seg = &c.segments;
            let mut pb = PathBuilder::new();
            let top = c.shape == Shape::Cross && seg.top;
            if top {
                push_rect(&mut pb, bar_lo, zlo - p.len - grow, bar_hi, zlo + grow);
            }
            if seg.bottom {
                push_rect(&mut pb, bar_lo, zhi - grow, bar_hi, zhi + p.len + grow);
            }
            if seg.left {
                push_rect(&mut pb, zlo - p.len - grow, bar_lo, zlo + grow, bar_hi);
            }
            if seg.right {
                push_rect(&mut pb, zhi - grow, bar_lo, zhi + p.len + grow, bar_hi);
            }
            fill(pb.finish(), FillRule::Winding);
            if c.center_dot {
                fill(dot_path(cx, cy, p.dot, grow, crisp), FillRule::Winding);
            }
        }
        Shape::Dot => {
            fill(dot_path(cx, cy, p.dot, grow, crisp), FillRule::Winding);
        }
        Shape::Circle | Shape::CircleDot => {
            let (ccx, ccy) = (cx + odd * 0.5, cy + odd * 0.5);
            let outer = p.radius + t / 2.0 + grow;
            let inner = p.radius - t / 2.0 - grow;
            let mut pb = PathBuilder::new();
            pb.push_circle(ccx, ccy, outer);
            if inner > 0.0 {
                pb.push_circle(ccx, ccy, inner);
            }
            fill(pb.finish(), FillRule::EvenOdd);
            if c.shape == Shape::CircleDot {
                fill(dot_path(cx, cy, p.dot, grow, crisp), FillRule::Winding);
            }
        }
        Shape::Square => {
            // ring width is exactly 2r + t; in pixel-perfect mode it is centred on a pixel for odd t
            let outer_lo = cx - p.radius - lo_side;
            let outer_hi = outer_lo + 2.0 * p.radius + t;
            let (ol, oh) = (outer_lo - grow, outer_hi + grow);
            let (il, ih) = (outer_lo + t + grow, outer_hi - t - grow);
            let mut pb = PathBuilder::new();
            push_rect(&mut pb, ol, ol, oh, oh);
            if il < ih {
                push_rect(&mut pb, il, il, ih, ih);
            }
            fill(pb.finish(), FillRule::EvenOdd);
            if c.center_dot {
                fill(dot_path(cx, cy, p.dot, grow, crisp), FillRule::Winding);
            }
        }
        Shape::Image => {}
    }
}

fn dot_path(cx: f32, cy: f32, dot: f32, grow: f32, crisp: bool) -> Option<Path> {
    if dot <= 0.0 {
        return None;
    }
    // odd diameters are centred on a pixel in pixel-perfect mode
    let off = if crisp && (dot as i64 & 1) == 1 {
        0.5
    } else {
        0.0
    };
    PathBuilder::from_circle(cx + off, cy + off, dot / 2.0 + grow)
}
