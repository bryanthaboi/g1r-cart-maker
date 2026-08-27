//! Shared app state: resolved paths and the settings file, behind one lock.

use crate::error::{AppError, AppResult};
use crate::paths::{clear_directory, directory_size, AppPaths};
use crate::settings::{self, Settings};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;

pub struct AppState {
    pub paths: AppPaths,
    settings: Mutex<Settings>,
    /// gh's own credential, asked for once. `None` means it was asked and had
    /// nothing to give; the outer Option is "not asked yet".
    gh_token: Mutex<Option<Option<String>>>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheUsage {
    pub feeds: u64,
    pub archives: u64,
    pub logs: u64,
    pub total: u64,
}

impl AppState {
    pub fn load() -> AppResult<Self> {
        let paths = AppPaths::resolve()?;
        let settings = settings::load(&paths);
        Ok(Self {
            paths,
            settings: Mutex::new(settings),
            gh_token: Mutex::new(None),
        })
    }

    /// The token API calls should carry: the usual environment variables first,
    /// then whatever `gh` is already signed in with, so nobody has to set
    /// GITHUB_TOKEN by hand to get past anonymous rate limits.
    ///
    /// Never log or surface the return value.
    pub fn github_token(&self) -> Option<String> {
        if let Some(from_env) = resolve::http::github_token(None) {
            return Some(from_env);
        }
        let mut cache = match self.gh_token.lock() {
            Ok(cache) => cache,
            Err(_) => return None,
        };
        if let Some(asked) = cache.as_ref() {
            return asked.clone();
        }
        let found = toolchain::detect::gh_token();
        *cache = Some(found.clone());
        found
    }

    /// Ask `gh` again, after the player signs in or out.
    pub fn forget_github_token(&self) {
        if let Ok(mut cache) = self.gh_token.lock() {
            *cache = None;
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn replace_settings(&self, next: Settings) -> AppResult<Settings> {
        let mut guard = self
            .settings
            .lock()
            .map_err(|_| AppError::new("io", "settings are locked"))?;
        *guard = next;
        settings::save(&self.paths, &guard)?;
        Ok(guard.clone())
    }

    pub fn mutate_settings<F>(&self, edit: F) -> AppResult<Settings>
    where
        F: FnOnce(&mut Settings) -> AppResult<()>,
    {
        let mut guard = self
            .settings
            .lock()
            .map_err(|_| AppError::new("io", "settings are locked"))?;
        edit(&mut guard)?;
        settings::save(&self.paths, &guard)?;
        Ok(guard.clone())
    }

    pub fn cache_usage(&self) -> CacheUsage {
        let feeds = directory_size(&self.paths.feeds);
        let archives = directory_size(&self.paths.archives);
        let logs = directory_size(&self.paths.logs);
        CacheUsage {
            feeds,
            archives,
            logs,
            total: feeds + archives + logs,
        }
    }

    pub fn clear_cache(&self, kind: &str) -> AppResult<CacheUsage> {
        match kind {
            "feeds" => clear_directory(&self.paths.feeds)?,
            "archives" => clear_directory(&self.paths.archives)?,
            "logs" => clear_directory(&self.paths.logs)?,
            "all" => {
                clear_directory(&self.paths.feeds)?;
                clear_directory(&self.paths.archives)?;
                clear_directory(&self.paths.logs)?;
            }
            other => return Err(AppError::invalid(format!("unknown cache {}", other))),
        }
        Ok(self.cache_usage())
    }

    /// Settings plus the recent-project list, as one JSON file the user keeps.
    pub fn export_data(&self, out_path: &Path) -> AppResult<String> {
        let payload = serde_json::json!({
            "exportedAt": settings::now_iso(),
            "settings": self.settings(),
            "paths": self.paths,
        });
        let mut body = serde_json::to_string_pretty(&payload)?;
        body.push('\n');
        settings::write_atomic(out_path, body.as_bytes())?;
        Ok(out_path.to_string_lossy().to_string())
    }
}

/// A stable id for an index source, derived from its feed URL.
pub fn source_id(feed: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(feed.as_bytes());
    digest
        .iter()
        .take(8)
        .map(|b| format!("{:02x}", b))
        .collect()
}
