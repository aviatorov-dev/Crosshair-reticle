//! App / tray icon drawn with tiny-skia (no binary assets needed).

use tiny_skia::{Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

/// Square icon: a ring with a dot and four ticks. `on = false` gives the grey "hidden" variant.
pub fn draw_icon(size: u32, on: bool) -> Pixmap {
    let mut pm = Pixmap::new(size, size).expect("icon size is valid");
    let s = size as f32;
    let c = s / 2.0;
    let (r, g, b) = if on { (76, 141, 255) } else { (120, 126, 140) };
    let mut paint = Paint::default();
    paint.set_color_rgba8(r, g, b, 255);
    paint.anti_alias = true;

    let ring = PathBuilder::from_circle(c, c, s * 0.33);
    if let Some(ring) = ring {
        let stroke = Stroke {
            width: s * 0.09,
            ..Stroke::default()
        };
        pm.stroke_path(&ring, &paint, &stroke, Transform::identity(), None);
    }
    if let Some(dot) = PathBuilder::from_circle(c, c, s * 0.07) {
        pm.fill_path(
            &dot,
            &paint,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    let w = s * 0.08;
    let tick = s * 0.17;
    let rects = [
        (c - w / 2.0, 0.0, c + w / 2.0, tick),
        (c - w / 2.0, s - tick, c + w / 2.0, s),
        (0.0, c - w / 2.0, tick, c + w / 2.0),
        (s - tick, c - w / 2.0, s, c + w / 2.0),
    ];
    for (l, t, rr, bb) in rects {
        if let Some(rc) = Rect::from_ltrb(l, t, rr, bb) {
            pm.fill_rect(rc, &paint, Transform::identity(), None);
        }
    }
    pm
}

/// Straight (non-premultiplied) RGBA bytes, as expected by tray / window icon APIs.
pub fn icon_rgba(size: u32, on: bool) -> Vec<u8> {
    let pm = draw_icon(size, on);
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for p in pm.pixels() {
        let c = p.demultiply();
        out.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    out
}
