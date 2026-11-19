//! Paths and logging (platform independent).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub config_file: PathBuf,
    pub backup_file: PathBuf,
    pub images_dir: PathBuf,
    pub logs_dir: PathBuf,
}

impl Paths {
    pub fn new(root: PathBuf) -> Self {
        Paths {
            config_file: root.join("config.toml"),
            backup_file: root.join("config.toml.bak"),
            images_dir: root.join("images"),
            logs_dir: root.join("logs"),
            root,
        }
    }

    /// `%APPDATA%\Reticle`
    pub fn default_dirs() -> Result<Self> {
        let base = directories::BaseDirs::new().ok_or_else(|| anyhow!("no home directory"))?;
        let p = Paths::new(base.config_dir().join("Reticle"));
        p.ensure()?;
        Ok(p)
    }

    pub fn ensure(&self) -> Result<()> {
        for d in [&self.root, &self.images_dir, &self.logs_dir] {
            fs::create_dir_all(d).with_context(|| format!("create {}", d.display()))?;
        }
        Ok(())
    }
}

/// Daily rolling log file in `logs/`. Keep the returned guard alive for the whole run.
pub fn init_logging(paths: &Paths) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let appender = tracing_appender::rolling::daily(&paths.logs_dir, "reticle.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let res = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .try_init();
    res.ok().map(|_| guard)
}

/// Small stable hash used to name imported images.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn file_name_of(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}
