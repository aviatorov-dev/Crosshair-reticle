//! "Start with Windows" via `HKCU\...\Run`.

use std::io;

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "Reticle";

pub fn set_enabled(enabled: bool) -> io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey_with_flags(RUN_KEY, KEY_WRITE)?;
    if enabled {
        let exe = std::env::current_exe()?;
        key.set_value(VALUE, &format!("\"{}\"", exe.display()))
    } else {
        match key.delete_value(VALUE) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

pub fn is_enabled() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_READ)
        .and_then(|k| k.get_value::<String, _>(VALUE))
        .is_ok()
}
