//! Every path the app writes to, in platform app-data directories. Nothing is
//! ever written outside these or a project directory the user chose.

use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const APP_DIR: &str = "g1r-cart-maker";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    pub config: PathBuf,
    pub cache: PathBuf,
    pub feeds: PathBuf,
    pub archives: PathBuf,
    pub logs: PathBuf,
    pub projects: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> AppResult<Self> {
        let config = dirs::config_dir()
            .ok_or_else(|| AppError::new("io", "no config directory on this system"))?
            .join(APP_DIR);
        let cache = dirs::cache_dir()
            .ok_or_else(|| AppError::new("io", "no cache directory on this system"))?
            .join(APP_DIR);
        let projects = dirs::document_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
            .join("G1R Carts");
        let paths = Self {
            feeds: cache.join("feeds"),
            archives: cache.join("archives"),
            logs: config.join("logs"),
            config,
            cache,
            projects,
        };
        paths.ensure()?;
        Ok(paths)
    }

    pub fn ensure(&self) -> AppResult<()> {
        for dir in [
            &self.config,
            &self.cache,
            &self.feeds,
            &self.archives,
            &self.logs,
        ] {
            fs::create_dir_all(dir).map_err(|problem| {
                AppError::io(format!("could not create {}", dir.display()), problem)
            })?;
        }
        Ok(())
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config.join("settings.json")
    }
}

pub fn directory_size(dir: &Path) -> u64 {
    let mut total = 0;
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    for entry in entries.flatten() {
        match entry.metadata() {
            Ok(meta) if meta.is_dir() => total += directory_size(&entry.path()),
            Ok(meta) => total += meta.len(),
            Err(_) => {}
        }
    }
    total
}

/// Empty a cache directory without removing the directory itself.
pub fn clear_directory(dir: &Path) -> AppResult<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let outcome = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        outcome.map_err(|problem| {
            AppError::io(format!("could not clear {}", path.display()), problem)
        })?;
    }
    Ok(())
}
