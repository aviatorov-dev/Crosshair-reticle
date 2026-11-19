use super::*;
use crate::config::{Outline, Rgba, Segments};

fn px(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = pm.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

fn solid(shape: Shape) -> Crosshair {
    Crosshair {
        shape,
        color: Rgba::new(255, 0, 0, 255),
        thickness: 2.0,
        length: 8.0,
        gap: 4.0,
        pixel_perfect: true,
        ..Crosshair::default()
    }
}

#[test]
fn canvas_is_even_square_and_contains_shape() {
    for shape in Shape::ALL {
        let c = Crosshair {
            shape,
            ..Crosshair::default()
        };
        let n = canvas_size(&c, 1.0, None);
        assert_eq!(n % 2, 0, "{shape:?}");
        assert!(n >= MIN_CANVAS);
        let pm = render(&c, 1.0, None);
        assert_eq!((pm.width(), pm.height()), (n, n));
    }
}

#[test]
fn canvas_grows_with_scale_and_rotation() {
    let c = Crosshair::default();
    assert!(canvas_size(&c, 2.0, None) > canvas_size(&c, 1.0, None));
    let r = Crosshair {
        rotation: 45.0,
        ..c.clone()
    };
    assert!(canvas_size(&r, 1.0, None) >= canvas_size(&c, 1.0, None));
}

#[test]
fn pixel_perfect_cross_even_thickness_exact_pixels() {
    // t=2, gap=4, len=8 -> vertical bar columns cx-1..cx, top arm rows cx-12..cx-5
    let c = solid(Shape::Cross);
    let pm = render(&c, 1.0, None);
    let cx = pm.width() / 2;
    assert_eq!(px(&pm, cx - 1, cx - 12), [255, 0, 0, 255]);
    assert_eq!(px(&pm, cx, cx - 5), [255, 0, 0, 255]);
    assert_eq!(px(&pm, cx - 2, cx - 8)[3], 0);
    assert_eq!(px(&pm, cx + 1, cx - 8)[3], 0);
    assert_eq!(px(&pm, cx, cx - 4)[3], 0, "gap must stay empty");
    assert_eq!(px(&pm, cx, cx - 13)[3], 0, "past the arm end");
    assert_eq!(px(&pm, cx, cx)[3], 0, "centre is empty with a gap");
}

#[test]
fn pixel_perfect_one_px_cross_is_centred_on_pixel() {
    let c = Crosshair {
        thickness: 1.0,
        length: 6.0,
        gap: 3.0,
        ..solid(Shape::Cross)
    };
    let pm = render(&c, 1.0, None);
    let cx = pm.width() / 2;
    // odd thickness: the line occupies exactly column cx, the empty zone is symmetric around it
    assert_eq!(px(&pm, cx, cx - 4)[3], 255);
    assert_eq!(px(&pm, cx, cx + 4)[3], 255);
    assert_eq!(px(&pm, cx, cx - 3)[3], 0);
    assert_eq!(px(&pm, cx, cx + 3)[3], 0);
    assert_eq!(px(&pm, cx - 4, cx)[3], 255);
    assert_eq!(px(&pm, cx + 4, cx)[3], 255);
    // no anti-aliasing: every pixel is fully on or off
    assert!(pm
        .pixels()
        .iter()
        .all(|p| p.alpha() == 0 || p.alpha() == 255));
}

#[test]
fn cross_is_symmetric() {
    let c = Crosshair {
        pixel_perfect: false,
        ..solid(Shape::Cross)
    };
    let pm = render(&c, 1.0, None);
    let n = pm.width();
    for y in 0..n {
        for x in 0..n {
            let a = px(&pm, x, y);
            let mirror_x = px(&pm, n - 1 - x, y);
            let mirror_y = px(&pm, x, n - 1 - y);
            assert_eq!(a, mirror_x);
            assert_eq!(a, mirror_y);
        }
    }
}

#[test]
fn segments_can_be_switched_off() {
    let c = Crosshair {
        segments: Segments {
            top: false,
            bottom: true,
            left: true,
            right: true,
        },
        ..solid(Shape::Cross)
    };
    let pm = render(&c, 1.0, None);
    let cx = pm.width() / 2;
    assert_eq!(px(&pm, cx, cx - 8)[3], 0);
    assert_eq!(px(&pm, cx, cx + 8)[3], 255);
}

#[test]
fn t_shape_has_no_top_arm() {
    let pm = render(&solid(Shape::T), 1.0, None);
    let cx = pm.width() / 2;
    assert_eq!(px(&pm, cx, cx - 8)[3], 0);
    assert_eq!(px(&pm, cx, cx + 8)[3], 255);
    assert_eq!(px(&pm, cx - 8, cx)[3], 255);
}

#[test]
fn dot_fills_centre() {
    let c = Crosshair {
        shape: Shape::Dot,
        dot_size: 6.0,
        ..solid(Shape::Dot)
    };
    let pm = render(&c, 1.0, None);
    let cx = pm.width() / 2;
    assert_eq!(px(&pm, cx, cx), [255, 0, 0, 255]);
    assert_eq!(px(&pm, cx - 1, cx - 1), [255, 0, 0, 255]);
    assert_eq!(px(&pm, cx + 5, cx)[3], 0);
}

#[test]
fn circle_is_hollow() {
    let c = Crosshair {
        shape: Shape::Circle,
        radius: 10.0,
        thickness: 2.0,
        ..solid(Shape::Circle)
    };
    let pm = render(&c, 1.0, None);
    let cx = pm.width() / 2;
    assert_eq!(px(&pm, cx, cx)[3], 0);
    assert_eq!(px(&pm, cx + 9, cx)[3], 255);
    let cd = Crosshair {
        shape: Shape::CircleDot,
        dot_size: 4.0,
        ..c
    };
    let pm = render(&cd, 1.0, None);
    assert_eq!(px(&pm, cx, cx)[3], 255);
}

#[test]
fn square_ring_is_hollow_with_exact_width() {
    let c = Crosshair {
        shape: Shape::Square,
        radius: 10.0,
        thickness: 2.0,
        ..solid(Shape::Square)
    };
    let pm = render(&c, 1.0, None);
    let cx = pm.width() / 2;
    assert_eq!(px(&pm, cx, cx)[3], 0);
    // outer edge at cx-11, inner edge at cx-9
    assert_eq!(px(&pm, cx - 11, cx)[3], 255);
    assert_eq!(px(&pm, cx - 10, cx)[3], 255);
    assert_eq!(px(&pm, cx - 9, cx)[3], 0);
    assert_eq!(px(&pm, cx - 12, cx)[3], 0);
}

#[test]
fn outline_is_outside_and_uses_its_colour() {
    let c = Crosshair {
        outline: Outline {
            enabled: true,
            color: Rgba::new(0, 0, 255, 255),
            thickness: 1.0,
            opacity: 1.0,
        },
        ..solid(Shape::Cross)
    };
    let plain = render(&solid(Shape::Cross), 1.0, None);
    let pm = render(&c, 1.0, None);
    // canvas is at least as large; compare around the same centre
    let (ca, cb) = (plain.width() / 2, pm.width() / 2);
    // fill pixel unchanged
    assert_eq!(px(&pm, cb, cb - 8), px(&plain, ca, ca - 8));
    // one pixel left of the bar is now blue
    assert_eq!(px(&pm, cb - 2, cb - 8), [0, 0, 255, 255]);
    assert_eq!(px(&plain, ca - 2, ca - 8)[3], 0);
}

#[test]
fn opacity_scales_alpha_and_stays_premultiplied() {
    let c = Crosshair {
        opacity: 0.5,
        ..solid(Shape::Dot)
    };
    let pm = render(
        &Crosshair {
            shape: Shape::Dot,
            dot_size: 6.0,
            ..c
        },
        1.0,
        None,
    );
    let cx = pm.width() / 2;
    let p = px(&pm, cx, cx);
    assert_eq!(p[3], 128);
    assert!(p[0] <= p[3], "premultiplied red must not exceed alpha");
    assert_eq!(p[0], 128);
}

#[test]
fn semi_transparent_overlap_is_not_doubled() {
    // gap 0: the four arms overlap in the middle; alpha must equal the plain arm alpha
    let c = Crosshair {
        gap: 0.0,
        color: Rgba::new(255, 255, 255, 128),
        ..solid(Shape::Cross)
    };
    let pm = render(&c, 1.0, None);
    let cx = pm.width() / 2;
    assert_eq!(px(&pm, cx, cx)[3], px(&pm, cx, cx - 5)[3]);
}

#[test]
fn rotation_turns_the_cross() {
    let c = Crosshair {
        rotation: 45.0,
        gap: 0.0,
        length: 10.0,
        pixel_perfect: false,
        ..solid(Shape::Cross)
    };
    let pm = render(&c, 1.0, None);
    let cx = pm.width() / 2;
    assert!(px(&pm, cx + 5, cx + 5)[3] > 200, "arm on the diagonal");
    assert_eq!(px(&pm, cx, cx - 8)[3], 0, "old vertical arm is gone");
}

#[test]
fn scale_makes_it_bigger() {
    let c = solid(Shape::Cross);
    let a = render(&c, 1.0, None);
    let b = render(&c, 2.0, None);
    let count = |pm: &Pixmap| pm.pixels().iter().filter(|p| p.alpha() > 0).count();
    assert!(count(&b) > count(&a) * 3);
}

#[test]
fn image_shape_draws_tinted_and_plain() {
    let mut img = RgbaImage::new(8, 8);
    for p in img.pixels_mut() {
        *p = image::Rgba([10, 200, 30, 255]);
    }
    let mut c = Crosshair {
        shape: Shape::Image,
        image: Some(crate::config::ImageSource {
            file: "x.png".into(),
            scale: 1.0,
            tint: None,
        }),
        ..Crosshair::default()
    };
    let pm = render(&c, 1.0, Some(&img));
    let cx = pm.width() / 2;
    assert_eq!(px(&pm, cx, cx), [10, 200, 30, 255]);
    c.image.as_mut().unwrap().tint = Some(Rgba::new(255, 0, 255, 255));
    let pm = render(&c, 1.0, Some(&img));
    assert_eq!(px(&pm, cx, cx), [255, 0, 255, 255]);
    // missing bitmap renders an empty canvas instead of panicking
    let pm = render(&c, 1.0, None);
    assert!(pm.pixels().iter().all(|p| p.alpha() == 0));
}

#[test]
fn deterministic() {
    let c = Crosshair {
        rotation: 30.0,
        ..Crosshair::default()
    };
    assert_eq!(render(&c, 1.5, None).data(), render(&c, 1.5, None).data());
}

#[test]
fn extreme_values_do_not_panic() {
    let mut c = Crosshair {
        length: 512.0,
        gap: 512.0,
        scale: 4.0,
        ..Crosshair::default()
    };
    c.sanitize();
    let pm = render(&c, 3.0, None);
    assert!(pm.width() <= MAX_CANVAS);
    let z = Crosshair {
        thickness: 0.0,
        length: 0.0,
        gap: 0.0,
        dot_size: 0.0,
        radius: 0.0,
        ..Crosshair::default()
    };
    let _ = render(&z, 1.0, None);
}
