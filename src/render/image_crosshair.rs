//! `Shape::Image`: a user PNG, centred, scaled, optionally tinted.

use image::RgbaImage;
use tiny_skia::{FilterQuality, IntSize, Pixmap, PixmapPaint, Transform};

use crate::config::Crosshair;

/// Straight RGBA image -> premultiplied pixmap.
pub fn premultiply_image(img: &RgbaImage) -> Option<Pixmap> {
    let mut data = img.as_raw().clone();
    for px in data.chunks_exact_mut(4) {
        let a = px[3] as u32;
        for c in &mut px[..3] {
            *c = ((*c as u32 * a + 127) / 255) as u8;
        }
    }
    Pixmap::from_vec(data, IntSize::from_wh(img.width(), img.height())?)
}

pub(super) fn render_image(
    c: &Crosshair,
    scale: f32,
    bitmap: Option<&RgbaImage>,
    n: u32,
) -> Pixmap {
    let mut out = Pixmap::new(n, n).expect("canvas size is valid");
    let (Some(img), Some(src)) = (bitmap, c.image.as_ref()) else {
        return out;
    };
    let Some(pm) = premultiply_image(img) else {
        return out;
    };
    let k = (c.scale * scale * src.scale).max(0.01);
    let centre = n as f32 / 2.0;
    let tf = Transform::from_translate(centre, centre)
        .pre_rotate(c.rotation)
        .pre_scale(k, k)
        .pre_translate(-(img.width() as f32) / 2.0, -(img.height() as f32) / 2.0);
    let paint = PixmapPaint {
        quality: FilterQuality::Bilinear,
        ..PixmapPaint::default()
    };
    out.draw_pixmap(0, 0, pm.as_ref(), &paint, tf, None);

    let opacity = c.opacity;
    let data = out.data_mut();
    match src.tint {
        Some(t) => {
            let [tr, tg, tb, ta] = t.0;
            for px in data.chunks_exact_mut(4) {
                // premultiplied source alpha is the shape; colour comes from the tint
                let a = px[3] as f32 / 255.0 * ta as f32 / 255.0 * opacity;
                px[0] = (tr as f32 * a).round() as u8;
                px[1] = (tg as f32 * a).round() as u8;
                px[2] = (tb as f32 * a).round() as u8;
                px[3] = (a * 255.0).round() as u8;
            }
        }
        None => {
            if opacity < 1.0 {
                for v in data.iter_mut() {
                    *v = (*v as f32 * opacity).round() as u8;
                }
            }
        }
    }
    out
}
