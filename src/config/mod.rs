//! Configuration: structures, loading / saving, migrations, presets, sharing.

pub mod migrate;
pub mod model;

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};

pub use model::*;

use crate::util::Paths;

const BUILTIN_TOML: &str = include_str!("../../assets/builtin_presets.toml");
pub const SHARE_PREFIX: &str = "RETICLE1:";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub active_preset: PresetId,
    pub general: General,
    pub hotkeys: Hotkeys,
    pub presets: Vec<Preset>,
}

#[derive(Deserialize)]
struct BuiltinFile {
    presets: Vec<Preset>,
}

/// The shipped presets (all marked `builtin`).
pub fn builtin_presets() -> Vec<Preset> {
    let mut f: BuiltinFile =
        toml::from_str(BUILTIN_TOML).expect("assets/builtin_presets.toml is valid");
    for p in &mut f.presets {
        p.builtin = true;
        p.crosshair.sanitize();
    }
    f.presets
}

impl Default for Config {
    fn default() -> Self {
        let presets = builtin_presets();
        Config {
            version: migrate::CURRENT_VERSION,
            active_preset: presets[0].id.clone(),
            general: General::default(),
            hotkeys: Hotkeys::default(),
            presets,
        }
    }
}

/// Result of `Config::load`.
pub struct Loaded {
    pub config: Config,
    /// Human readable problem (corrupt file was backed up and reset), if any.
    pub notice: Option<String>,
}

impl Config {
    // ------------------------------------------------------------ io

    /// Never fails: a broken file is copied to `config.toml.corrupt.bak` and defaults are used.
    pub fn load(paths: &Paths) -> Loaded {
        let text = match fs::read_to_string(&paths.config_file) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Loaded {
                    config: Config::default(),
                    notice: None,
                };
            }
            Err(e) => return Self::reset_after_error(paths, anyhow!(e)),
        };
        match Self::parse(&text) {
            Ok((cfg, migrated)) => {
                if migrated {
                    // keep a copy of the old format before it is overwritten
                    let _ = fs::copy(&paths.config_file, &paths.backup_file);
                    let _ = cfg.save(paths);
                }
                Loaded {
                    config: cfg,
                    notice: None,
                }
            }
            Err(e) => Self::reset_after_error(paths, e),
        }
    }

    fn reset_after_error(paths: &Paths, err: anyhow::Error) -> Loaded {
        tracing::error!("config is unusable: {err:#}");
        let corrupt = paths.config_file.with_extension("toml.corrupt.bak");
        let _ = fs::copy(&paths.config_file, &corrupt);
        let cfg = Config::default();
        let _ = cfg.save(paths);
        Loaded {
            config: cfg,
            notice: Some(format!("{err:#}")),
        }
    }

    /// Returns the config and whether a migration happened.
    pub fn parse(text: &str) -> Result<(Config, bool)> {
        let doc: toml::Value = toml::from_str(text).context("invalid TOML")?;
        let migrated = migrate::needs_migration(&doc);
        let doc = migrate::migrate(doc)?;
        let mut cfg: Config = doc.try_into().context("invalid config structure")?;
        cfg.validate();
        Ok((cfg, migrated))
    }

    /// Atomic write: temp file in the same folder, then rename over the target.
    pub fn save(&self, paths: &Paths) -> Result<()> {
        let text = toml::to_string_pretty(self).context("serialize config")?;
        if let Some(dir) = paths.config_file.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = paths.config_file.with_extension("toml.tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &paths.config_file)?;
        Ok(())
    }

    /// Repairs everything that could be wrong with a hand-edited file.
    pub fn validate(&mut self) {
        self.version = migrate::CURRENT_VERSION;
        self.general.ui_scale = if self.general.ui_scale.is_finite() {
            self.general.ui_scale.clamp(0.8, 1.5)
        } else {
            1.0
        };
        self.general.recent_colors.truncate(8);
        if let Some(sz) = &mut self.general.window_size {
            sz[0] = if sz[0].is_finite() {
                sz[0].clamp(720.0, 4000.0)
            } else {
                860.0
            };
            sz[1] = if sz[1].is_finite() {
                sz[1].clamp(480.0, 4000.0)
            } else {
                560.0
            };
        }
        for p in &mut self.presets {
            p.crosshair.sanitize();
            if let Some(h) = &p.hotkey {
                if crate::hotkeys::parse(h).is_err() {
                    p.hotkey = None;
                }
            }
        }
        // unique ids
        let mut seen = std::collections::HashSet::new();
        self.presets.retain(|p| seen.insert(p.id.clone()));
        // every built-in preset must exist
        for b in builtin_presets() {
            match self.presets.iter_mut().find(|p| p.id == b.id) {
                Some(p) => p.builtin = true,
                None => self.presets.push(b),
            }
        }
        if !self.presets.iter().any(|p| p.id == self.active_preset) {
            self.active_preset = self.presets[0].id.clone();
        }
        if crate::hotkeys::parse(&self.hotkeys.toggle).is_err() {
            self.hotkeys.toggle = Hotkeys::default().toggle;
        }
        for slot in [
            &mut self.hotkeys.open_settings,
            &mut self.hotkeys.next_preset,
            &mut self.hotkeys.prev_preset,
        ] {
            if slot
                .as_deref()
                .map(crate::hotkeys::parse)
                .transpose()
                .is_err()
            {
                *slot = None;
            }
        }
    }

    // ------------------------------------------------------- presets

    pub fn preset(&self, id: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.id == id)
    }

    pub fn preset_mut(&mut self, id: &str) -> Option<&mut Preset> {
        self.presets.iter_mut().find(|p| p.id == id)
    }

    pub fn active(&self) -> &Preset {
        self.preset(&self.active_preset).unwrap_or(&self.presets[0])
    }

    pub fn active_mut(&mut self) -> &mut Preset {
        let id = self.active_preset.clone();
        let idx = self.presets.iter().position(|p| p.id == id).unwrap_or(0);
        &mut self.presets[idx]
    }

    pub fn set_active(&mut self, id: &str) -> bool {
        if self.preset(id).is_some() && self.active_preset != id {
            self.active_preset = id.to_string();
            true
        } else {
            false
        }
    }

    /// `dir` = +1 next, -1 previous (cyclic).
    pub fn cycle_active(&mut self, dir: i32) {
        let n = self.presets.len() as i32;
        if n == 0 {
            return;
        }
        let cur = self
            .presets
            .iter()
            .position(|p| p.id == self.active_preset)
            .unwrap_or(0) as i32;
        let next = (cur + dir).rem_euclid(n) as usize;
        self.active_preset = self.presets[next].id.clone();
    }

    pub fn unique_id(&self, base: &str) -> PresetId {
        let slug: String = base
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let slug = slug.trim_matches('-');
        let slug = if slug.is_empty() { "preset" } else { slug };
        if self.preset(slug).is_none() {
            return slug.to_string();
        }
        (2..)
            .map(|n| format!("{slug}-{n}"))
            .find(|id| self.preset(id).is_none())
            .unwrap()
    }

    /// Adds a user preset; returns its id.
    pub fn add_preset(&mut self, name: &str, crosshair: Crosshair) -> PresetId {
        let id = self.unique_id(name);
        self.presets.push(Preset {
            id: id.clone(),
            name: name.to_string(),
            builtin: false,
            hotkey: None,
            crosshair,
        });
        id
    }

    pub fn duplicate_preset(&mut self, id: &str, copy_suffix: &str) -> Option<PresetId> {
        let src = self.preset(id)?.clone();
        let name = format!("{} {}", src.name, copy_suffix);
        let new_id = self.add_preset(&name, src.crosshair);
        Some(new_id)
    }

    pub fn rename_preset(&mut self, id: &str, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(p) = self.preset_mut(id) {
            p.name = name.to_string();
        }
    }

    /// Built-in presets cannot be deleted. Returns the removed preset and its index (for undo).
    pub fn delete_preset(&mut self, id: &str) -> Option<(Preset, usize)> {
        let idx = self.presets.iter().position(|p| p.id == id)?;
        if self.presets[idx].builtin {
            return None;
        }
        let removed = self.presets.remove(idx);
        if self.active_preset == id {
            let ni = idx.saturating_sub(1).min(self.presets.len() - 1);
            self.active_preset = self.presets[ni].id.clone();
        }
        Some((removed, idx))
    }

    pub fn restore_preset(&mut self, p: Preset, idx: usize) {
        if self.preset(&p.id).is_none() {
            let idx = idx.min(self.presets.len());
            self.presets.insert(idx, p);
        }
    }

    /// Resets one preset: built-ins go back to their shipped look, user presets to defaults.
    pub fn reset_preset(&mut self, id: &str) {
        let original = builtin_presets()
            .into_iter()
            .find(|b| b.id == id)
            .map(|b| b.crosshair);
        if let Some(p) = self.preset_mut(id) {
            p.crosshair = original.unwrap_or_default();
        }
    }

    /// All hotkeys as `(action key, combo)`. Action keys: `toggle`, `open_settings`,
    /// `next_preset`, `prev_preset`, `preset:<id>`.
    pub fn hotkey_bindings(&self) -> Vec<(String, String)> {
        let mut v = vec![("toggle".to_string(), self.hotkeys.toggle.clone())];
        for (action, combo) in [
            ("open_settings", &self.hotkeys.open_settings),
            ("next_preset", &self.hotkeys.next_preset),
            ("prev_preset", &self.hotkeys.prev_preset),
        ] {
            if let Some(combo) = combo {
                v.push((action.to_string(), combo.clone()));
            }
        }
        for p in &self.presets {
            if let Some(h) = &p.hotkey {
                v.push((format!("preset:{}", p.id), h.clone()));
            }
        }
        v
    }

    /// Writes a combo into the slot named by an action key.
    pub fn set_hotkey(&mut self, action: &str, combo: Option<String>) {
        match action {
            "toggle" => self.hotkeys.toggle = combo.unwrap_or_else(|| Hotkeys::default().toggle),
            "open_settings" => self.hotkeys.open_settings = combo,
            "next_preset" => self.hotkeys.next_preset = combo,
            "prev_preset" => self.hotkeys.prev_preset = combo,
            other => {
                if let Some(id) = other.strip_prefix("preset:") {
                    if let Some(p) = self.preset_mut(id) {
                        p.hotkey = combo;
                    }
                }
            }
        }
    }

    pub fn push_recent_color(&mut self, c: Rgba) {
        let r = &mut self.general.recent_colors;
        r.retain(|x| *x != c);
        r.insert(0, c);
        r.truncate(8);
    }
}

// ----------------------------------------------------------- sharing

/// A preset plus (optionally) the PNG it uses, so it can be moved between machines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedPreset {
    pub reticle_preset: u32,
    pub preset: Preset,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png_base64: Option<String>,
}

impl SharedPreset {
    pub fn new(preset: &Preset, png: Option<Vec<u8>>) -> Self {
        let mut preset = preset.clone();
        preset.hotkey = None;
        preset.builtin = false;
        SharedPreset {
            reticle_preset: 1,
            preset,
            png_base64: png.map(|b| base64::engine::general_purpose::STANDARD.encode(b)),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("preset serializes")
    }

    /// Compact single-line form for the clipboard.
    pub fn to_share_string(&self) -> String {
        let json = serde_json::to_string(self).expect("preset serializes");
        format!(
            "{SHARE_PREFIX}{}",
            base64::engine::general_purpose::STANDARD.encode(json)
        )
    }

    /// Accepts a JSON document or a `RETICLE1:` string.
    pub fn parse(text: &str) -> Result<Self> {
        let t = text.trim();
        let json: String = if let Some(b64) = t.strip_prefix(SHARE_PREFIX) {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64.trim())
                .map_err(|e| anyhow!("bad share string: {e}"))?;
            String::from_utf8(bytes).map_err(|e| anyhow!("bad share string: {e}"))?
        } else if t.starts_with('{') {
            t.to_string()
        } else {
            bail!("not a Crosshair reticle preset");
        };
        let mut s: SharedPreset = serde_json::from_str(&json).context("invalid preset JSON")?;
        if s.reticle_preset != 1 {
            bail!("unsupported preset format {}", s.reticle_preset);
        }
        s.preset.crosshair.sanitize();
        s.preset.builtin = false;
        s.preset.hotkey = None;
        if s.preset.name.trim().is_empty() {
            s.preset.name = "Imported".into();
        }
        Ok(s)
    }

    pub fn png_bytes(&self) -> Option<Vec<u8>> {
        let b = self.png_base64.as_ref()?;
        base64::engine::general_purpose::STANDARD.decode(b).ok()
    }
}

/// Convenience for tests / callers that only have a path.
pub fn read_text(path: &Path) -> Result<String> {
    Ok(fs::read_to_string(path)?)
}

impl SharedPreset {
    /// Like `parse`, but also accepts a JSON array of presets (the "export all" file).
    pub fn parse_many(text: &str) -> Result<Vec<SharedPreset>> {
        let t = text.trim();
        if t.starts_with('[') {
            let mut v: Vec<SharedPreset> =
                serde_json::from_str(t).context("invalid preset list")?;
            for s in &mut v {
                if s.reticle_preset != 1 {
                    bail!("unsupported preset format {}", s.reticle_preset);
                }
                s.preset.crosshair.sanitize();
                s.preset.builtin = false;
                s.preset.hotkey = None;
            }
            if v.is_empty() {
                bail!("the file contains no presets");
            }
            Ok(v)
        } else {
            Ok(vec![Self::parse(t)?])
        }
    }
}
