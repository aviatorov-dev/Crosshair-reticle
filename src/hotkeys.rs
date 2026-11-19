//! Hotkey strings such as `Ctrl+Alt+X`: parsing and formatting (platform independent).

use thiserror::Error;

pub const MOD_ALT: u32 = 0x1;
pub const MOD_CONTROL: u32 = 0x2;
pub const MOD_SHIFT: u32 = 0x4;
pub const MOD_WIN: u32 = 0x8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    pub mods: u32,
    /// Windows virtual-key code.
    pub vk: u32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HotkeyError {
    #[error("empty hotkey")]
    Empty,
    #[error("unknown key: {0}")]
    UnknownKey(String),
    #[error("a modifier (Ctrl, Alt, Shift, Win) is required")]
    NeedsModifier,
    #[error("the combination has no main key")]
    NoKey,
    #[error("more than one main key")]
    TooManyKeys,
}

/// (canonical name, vk, accepted aliases in lower case)
const NAMED: &[(&str, u32, &[&str])] = &[
    ("Left", 0x25, &["arrowleft"]),
    ("Up", 0x26, &["arrowup"]),
    ("Right", 0x27, &["arrowright"]),
    ("Down", 0x28, &["arrowdown"]),
    ("Space", 0x20, &[]),
    ("Enter", 0x0D, &["return"]),
    ("Tab", 0x09, &[]),
    ("Backspace", 0x08, &["back"]),
    ("Delete", 0x2E, &["del"]),
    ("Insert", 0x2D, &["ins"]),
    ("Home", 0x24, &[]),
    ("End", 0x23, &[]),
    ("PageUp", 0x21, &["pgup"]),
    ("PageDown", 0x22, &["pgdown", "pgdn"]),
    ("Esc", 0x1B, &["escape"]),
    ("Minus", 0xBD, &["-"]),
    ("Plus", 0xBB, &["equals", "=", "+"]),
    ("Comma", 0xBC, &[","]),
    ("Period", 0xBE, &["."]),
    ("Slash", 0xBF, &["/"]),
    ("Backslash", 0xDC, &["\\"]),
    ("Semicolon", 0xBA, &[";"]),
    ("Quote", 0xDE, &["'"]),
    ("Backtick", 0xC0, &["`", "backquote", "grave"]),
    ("OpenBracket", 0xDB, &["["]),
    ("CloseBracket", 0xDD, &["]"]),
];

pub fn key_vk(name: &str) -> Option<u32> {
    let n = name.trim().to_ascii_lowercase();
    if n.len() == 1 {
        let c = n.as_bytes()[0];
        if c.is_ascii_lowercase() {
            return Some(c.to_ascii_uppercase() as u32);
        }
        if c.is_ascii_digit() {
            return Some(c as u32);
        }
    }
    // F1..F24
    if let Some(num) = n.strip_prefix('f') {
        if let Ok(k) = num.parse::<u32>() {
            if (1..=24).contains(&k) {
                return Some(0x6F + k);
            }
        }
    }
    // egui names digits "Num0".."Num9"
    if let Some(d) = n.strip_prefix("num") {
        if d.len() == 1 && d.as_bytes()[0].is_ascii_digit() {
            return Some(d.as_bytes()[0] as u32);
        }
    }
    if let Some(d) = n.strip_prefix("numpad") {
        if d.len() == 1 && d.as_bytes()[0].is_ascii_digit() {
            return Some(0x60 + (d.as_bytes()[0] - b'0') as u32);
        }
    }
    for (canon, vk, aliases) in NAMED {
        if canon.eq_ignore_ascii_case(&n) || aliases.contains(&n.as_str()) {
            return Some(*vk);
        }
    }
    None
}

pub fn vk_name(vk: u32) -> Option<String> {
    match vk {
        0x41..=0x5A | 0x30..=0x39 => Some((vk as u8 as char).to_string()),
        0x70..=0x87 => Some(format!("F{}", vk - 0x6F)),
        0x60..=0x69 => Some(format!("Numpad{}", vk - 0x60)),
        _ => NAMED
            .iter()
            .find(|(_, v, _)| *v == vk)
            .map(|(n, _, _)| n.to_string()),
    }
}

pub fn parse(s: &str) -> Result<Hotkey, HotkeyError> {
    if s.trim().is_empty() {
        return Err(HotkeyError::Empty);
    }
    let mut mods = 0;
    let mut vk: Option<u32> = None;
    // "Ctrl+Plus" is ambiguous with the separator, so split on '+' but keep a trailing "+" key
    let normalized = s.replace("++", "+Plus");
    for tok in normalized.split('+') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        match t.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "ctl" => mods |= MOD_CONTROL,
            "alt" | "option" => mods |= MOD_ALT,
            "shift" => mods |= MOD_SHIFT,
            "win" | "super" | "meta" | "cmd" | "windows" => mods |= MOD_WIN,
            _ => {
                let k = key_vk(t).ok_or_else(|| HotkeyError::UnknownKey(t.to_string()))?;
                if vk.replace(k).is_some() {
                    return Err(HotkeyError::TooManyKeys);
                }
            }
        }
    }
    let vk = vk.ok_or(HotkeyError::NoKey)?;
    let is_f_key = (0x70..=0x87).contains(&vk);
    if mods == 0 && !is_f_key {
        return Err(HotkeyError::NeedsModifier);
    }
    Ok(Hotkey { mods, vk })
}

pub fn format(h: &Hotkey) -> String {
    let mut parts: Vec<String> = Vec::new();
    if h.mods & MOD_CONTROL != 0 {
        parts.push("Ctrl".into());
    }
    if h.mods & MOD_ALT != 0 {
        parts.push("Alt".into());
    }
    if h.mods & MOD_SHIFT != 0 {
        parts.push("Shift".into());
    }
    if h.mods & MOD_WIN != 0 {
        parts.push("Win".into());
    }
    parts.push(vk_name(h.vk).unwrap_or_else(|| format!("0x{:X}", h.vk)));
    parts.join("+")
}

/// Normalises a hotkey string (`ctrl+alt+x` -> `Ctrl+Alt+X`).
pub fn canonical(s: &str) -> Option<String> {
    parse(s).ok().map(|h| format(&h))
}

/// Builds a hotkey string from a modifier set and the Debug name of an egui `Key`.
pub fn from_parts(ctrl: bool, alt: bool, shift: bool, win: bool, key_name: &str) -> Option<String> {
    let vk = key_vk(key_name)?;
    let mut mods = 0;
    if ctrl {
        mods |= MOD_CONTROL;
    }
    if alt {
        mods |= MOD_ALT;
    }
    if shift {
        mods |= MOD_SHIFT;
    }
    if win {
        mods |= MOD_WIN;
    }
    let h = Hotkey { mods, vk };
    let s = format(&h);
    parse(&s).ok().map(|_| s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic() {
        let h = parse("Ctrl+Alt+X").unwrap();
        assert_eq!(h.mods, MOD_CONTROL | MOD_ALT);
        assert_eq!(h.vk, b'X' as u32);
        assert_eq!(format(&h), "Ctrl+Alt+X");
    }

    #[test]
    fn case_and_aliases() {
        assert_eq!(
            canonical("ctrl + alt + arrowright").unwrap(),
            "Ctrl+Alt+Right"
        );
        assert_eq!(canonical("SHIFT+f5").unwrap(), "Shift+F5");
        assert_eq!(canonical("control+Num1").unwrap(), "Ctrl+1");
    }

    #[test]
    fn plus_key() {
        let h = parse("Ctrl++").unwrap();
        assert_eq!(h.vk, 0xBB);
    }

    #[test]
    fn errors() {
        assert_eq!(parse(""), Err(HotkeyError::Empty));
        assert_eq!(parse("Ctrl+"), Err(HotkeyError::NoKey));
        assert_eq!(parse("X"), Err(HotkeyError::NeedsModifier));
        assert!(matches!(parse("Ctrl+Foo"), Err(HotkeyError::UnknownKey(_))));
        assert_eq!(parse("Ctrl+A+B"), Err(HotkeyError::TooManyKeys));
        assert!(parse("F8").is_ok());
    }

    #[test]
    fn roundtrip_all_named() {
        for (name, vk, _) in NAMED {
            let s = format!("Ctrl+{name}");
            let h = parse(&s).unwrap();
            assert_eq!(h.vk, *vk, "{s}");
            assert_eq!(format(&h), s);
        }
    }

    #[test]
    fn from_egui_names() {
        assert_eq!(
            from_parts(true, true, false, false, "ArrowLeft").unwrap(),
            "Ctrl+Alt+Left"
        );
        assert_eq!(
            from_parts(true, false, false, false, "Num5").unwrap(),
            "Ctrl+5"
        );
        assert!(from_parts(false, false, false, false, "A").is_none());
    }
}
