//! Config format migrations. Add a new arm to `step` whenever `CURRENT_VERSION` is bumped.

use anyhow::{bail, Result};

pub const CURRENT_VERSION: u32 = 2;

/// Brings an arbitrary (older) config document up to `CURRENT_VERSION`.
pub fn migrate(mut doc: toml::Value) -> Result<toml::Value> {
    let table = match doc.as_table_mut() {
        Some(t) => t,
        None => bail!("config root is not a table"),
    };
    let mut version = table
        .get("version")
        .and_then(|v| v.as_integer())
        .unwrap_or(0)
        .max(0) as u32;
    if version > CURRENT_VERSION {
        bail!("config version {version} is newer than supported {CURRENT_VERSION}");
    }
    while version < CURRENT_VERSION {
        step(table, version);
        version += 1;
        table.insert("version".into(), toml::Value::Integer(version as i64));
    }
    Ok(doc)
}

fn step(table: &mut toml::map::Map<String, toml::Value>, from: u32) {
    // 0 -> 1: files without a `version` key. Every field has a serde default, nothing to rewrite.
    if from == 1 {
        // 1 -> 2: clear old factory shortcuts, but keep user-customized bindings.
        if let Some(hotkeys) = table.get_mut("hotkeys").and_then(toml::Value::as_table_mut) {
            for (name, old_default) in [
                ("open_settings", "Ctrl+Alt+S"),
                ("next_preset", "Ctrl+Alt+Right"),
                ("prev_preset", "Ctrl+Alt+Left"),
            ] {
                if hotkeys.get(name).and_then(toml::Value::as_str) == Some(old_default) {
                    hotkeys.remove(name);
                }
            }
        }
    }
}

/// True if the document needs a rewrite (older than the current format).
pub fn needs_migration(doc: &toml::Value) -> bool {
    let v = doc.get("version").and_then(|v| v.as_integer()).unwrap_or(0);
    (v as u32) < CURRENT_VERSION
}
