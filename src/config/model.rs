//! Data model of the configuration (section 5.5 of the spec).

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub type PresetId = String;

// ---------------------------------------------------------------- colour

/// Straight (non-premultiplied) RGBA colour, serialised as `#RRGGBBAA`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Rgba([r, g, b, a])
    }

    pub fn to_hex(self) -> String {
        let [r, g, b, a] = self.0;
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }

    /// Accepts `#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA` (the `#` is optional).
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('#');
        if !s.is_ascii() {
            return None;
        }
        let nib = |c: u8| (c as char).to_digit(16).map(|v| v as u8);
        let b = s.as_bytes();
        match b.len() {
            3 | 4 => {
                let mut v = [0u8, 0, 0, 255];
                for (i, c) in b.iter().enumerate() {
                    let n = nib(*c)?;
                    v[i] = n * 17;
                }
                Some(Rgba(v))
            }
            6 | 8 => {
                let mut v = [0u8, 0, 0, 255];
                for i in 0..b.len() / 2 {
                    v[i] = nib(b[2 * i])? * 16 + nib(b[2 * i + 1])?;
                }
                Some(Rgba(v))
            }
            _ => None,
        }
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Rgba::from_hex(&s).ok_or_else(|| serde::de::Error::custom(format!("bad colour: {s}")))
    }
}

// ----------------------------------------------------------------- enums

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    #[default]
    Cross,
    Dot,
    Circle,
    CircleDot,
    T,
    Square,
    Image,
}

impl Shape {
    pub const ALL: [Shape; 7] = [
        Shape::Cross,
        Shape::Dot,
        Shape::Circle,
        Shape::CircleDot,
        Shape::T,
        Shape::Square,
        Shape::Image,
    ];

    pub fn label(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Shape::Cross, Lang::Ru) => "Крест",
            (Shape::Cross, Lang::En) => "Cross",
            (Shape::Dot, Lang::Ru) => "Точка",
            (Shape::Dot, Lang::En) => "Dot",
            (Shape::Circle, Lang::Ru) => "Круг",
            (Shape::Circle, Lang::En) => "Circle",
            (Shape::CircleDot, Lang::Ru) => "Круг с точкой",
            (Shape::CircleDot, Lang::En) => "Circle + dot",
            (Shape::T, Lang::Ru) => "Т-образный",
            (Shape::T, Lang::En) => "T-shape",
            (Shape::Square, Lang::Ru) => "Квадрат",
            (Shape::Square, Lang::En) => "Square",
            (Shape::Image, Lang::Ru) => "Свой PNG",
            (Shape::Image, Lang::En) => "Custom PNG",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    #[default]
    Ru,
    En,
}

/// `"primary"` or the Windows device name (`\\.\DISPLAY2`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum MonitorChoice {
    #[default]
    Primary,
    Named(String),
}

impl From<String> for MonitorChoice {
    fn from(s: String) -> Self {
        if s.eq_ignore_ascii_case("primary") || s.trim().is_empty() {
            MonitorChoice::Primary
        } else {
            MonitorChoice::Named(s)
        }
    }
}

impl From<MonitorChoice> for String {
    fn from(m: MonitorChoice) -> Self {
        match m {
            MonitorChoice::Primary => "primary".to_string(),
            MonitorChoice::Named(n) => n,
        }
    }
}

// ------------------------------------------------------------- crosshair

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Outline {
    pub enabled: bool,
    pub color: Rgba,
    pub thickness: f32,
    pub opacity: f32,
}

impl Default for Outline {
    fn default() -> Self {
        Outline {
            enabled: false,
            color: Rgba::new(0, 0, 0, 255),
            thickness: 1.0,
            opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Segments {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}

impl Default for Segments {
    fn default() -> Self {
        Segments {
            top: true,
            bottom: true,
            left: true,
            right: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageSource {
    /// File name inside `%APPDATA%\Reticle\images\`.
    pub file: String,
    /// Extra scale on top of `Crosshair::scale`.
    pub scale: f32,
    /// If set, the image is recoloured with this colour (its alpha is kept).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tint: Option<Rgba>,
}

impl Default for ImageSource {
    fn default() -> Self {
        ImageSource {
            file: String::new(),
            scale: 1.0,
            tint: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Crosshair {
    pub shape: Shape,
    pub color: Rgba,
    pub thickness: f32,
    pub length: f32,
    pub gap: f32,
    pub dot_size: f32,
    pub radius: f32,
    /// Draw a dot in the middle of Cross / T / Square (`dot_size` px).
    pub center_dot: bool,
    pub outline: Outline,
    pub opacity: f32,
    pub scale: f32,
    pub rotation: f32,
    pub offset: (i32, i32),
    pub segments: Segments,
    /// Hard pixel edges, no anti-aliasing (ignored while rotated).
    pub pixel_perfect: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageSource>,
}

impl Default for Crosshair {
    fn default() -> Self {
        Crosshair {
            shape: Shape::Cross,
            color: Rgba::new(255, 59, 59, 255),
            thickness: 2.0,
            length: 8.0,
            gap: 4.0,
            dot_size: 4.0,
            radius: 12.0,
            center_dot: false,
            outline: Outline::default(),
            opacity: 1.0,
            scale: 1.0,
            rotation: 0.0,
            offset: (0, 0),
            segments: Segments::default(),
            pixel_perfect: false,
            image: None,
        }
    }
}

fn fix(v: &mut f32, lo: f32, hi: f32, default: f32) {
    *v = if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        default
    };
}

impl Crosshair {
    /// Clamp every numeric field into its valid range (used after loading / importing).
    pub fn sanitize(&mut self) {
        let d = Crosshair::default();
        fix(&mut self.thickness, 0.5, 64.0, d.thickness);
        fix(&mut self.length, 0.0, 512.0, d.length);
        fix(&mut self.gap, 0.0, 512.0, d.gap);
        fix(&mut self.dot_size, 0.0, 128.0, d.dot_size);
        fix(&mut self.radius, 1.0, 512.0, d.radius);
        fix(&mut self.outline.thickness, 0.0, 16.0, 1.0);
        fix(&mut self.outline.opacity, 0.0, 1.0, 1.0);
        fix(&mut self.opacity, 0.0, 1.0, 1.0);
        fix(&mut self.scale, 0.25, 4.0, 1.0);
        fix(&mut self.rotation, -360.0, 360.0, 0.0);
        self.offset.0 = self.offset.0.clamp(-4000, 4000);
        self.offset.1 = self.offset.1.clamp(-4000, 4000);
        if let Some(img) = &mut self.image {
            fix(&mut img.scale, 0.05, 10.0, 1.0);
        }
    }
}

// ---------------------------------------------------------------- preset

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub id: PresetId,
    pub name: String,
    #[serde(default)]
    pub builtin: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<String>,
    #[serde(default)]
    pub crosshair: Crosshair,
}

// ------------------------------------------------------- global settings

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    pub toggle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_settings: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_preset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_preset: Option<String>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Hotkeys {
            toggle: "Ctrl+Alt+X".into(),
            open_settings: None,
            next_preset: None,
            prev_preset: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    pub monitor: MonitorChoice,
    pub autostart: bool,
    pub start_minimized: bool,
    pub theme: ThemeMode,
    pub language: Lang,
    /// Interface zoom, 0.8 ..= 1.5.
    pub ui_scale: f32,
    pub reduce_motion: bool,
    /// Multiply crosshair size by the monitor's DPI factor.
    pub scale_with_dpi: bool,
    pub show_crosshair: bool,
    pub hide_crosshair_on_rmb: bool,
    pub onboarded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_size: Option<[f32; 2]>,
    pub recent_colors: Vec<Rgba>,
}

impl Default for General {
    fn default() -> Self {
        General {
            monitor: MonitorChoice::Primary,
            autostart: false,
            start_minimized: false,
            theme: ThemeMode::Dark,
            language: Lang::Ru,
            ui_scale: 1.0,
            reduce_motion: false,
            scale_with_dpi: true,
            show_crosshair: true,
            hide_crosshair_on_rmb: false,
            onboarded: false,
            window_size: None,
            recent_colors: Vec::new(),
        }
    }
}
