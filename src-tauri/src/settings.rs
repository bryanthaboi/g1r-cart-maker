//! Persisted app configuration. Never holds a credential: gh owns those.

use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const BUILTIN_SOURCE_ID: &str = "gen1recomp-mod-index";
pub const BUILTIN_SOURCE_URL: &str = "bryanthaboi/gen1recomp-mod-index";
/// The engine's mod API major (src/core/Version.lua `modApi`).
pub const DEFAULT_MOD_API: u32 = 2;
/// Fallback only: the app resolves the engine's latest release when asked.
pub const DEFAULT_ENGINE_VERSION: &str = "0.2.26";
pub const ENGINE_REPO: &str = "bryanthaboi/gen1recomp";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexSourceSetting {
    pub id: String,
    pub url: String,
    pub feed: String,
    pub base: String,
    pub fallback: Option<String>,
    pub label: String,
    pub enabled: bool,
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentProject {
    pub path: String,
    pub id: String,
    pub title: String,
    pub base: String,
    pub opened_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub engine_version: String,
    pub mod_api: u32,
    pub index_sources: Vec<IndexSourceSetting>,
    pub recent_projects: Vec<RecentProject>,
    pub cache_ttl_hours: u64,
    pub theme: String,
    /// Set when the player deliberately removes the shipped index source.
    #[serde(default)]
    pub builtin_removed: bool,
    /// The player's game directory, for reading mod_option_schemas.json.
    pub game_path: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            engine_version: DEFAULT_ENGINE_VERSION.to_string(),
            mod_api: DEFAULT_MOD_API,
            index_sources: vec![builtin_source()],
            recent_projects: Vec::new(),
            cache_ttl_hours: 24,
            theme: "system".to_string(),
            builtin_removed: false,
            game_path: None,
        }
    }
}

pub fn builtin_source() -> IndexSourceSetting {
    let resolved = cartcore::index::resolve_source(BUILTIN_SOURCE_URL)
        .expect("the builtin index source always resolves");
    IndexSourceSetting {
        id: BUILTIN_SOURCE_ID.to_string(),
        url: BUILTIN_SOURCE_URL.to_string(),
        feed: resolved.feed,
        base: resolved.base,
        fallback: resolved.fallback,
        label: resolved.label,
        enabled: true,
        builtin: true,
    }
}

pub fn load(paths: &AppPaths) -> Settings {
    let path = paths.settings_file();
    let body = match fs::read_to_string(&path) {
        Ok(body) => body,
        Err(_) => return Settings::default(),
    };
    let mut settings: Settings = serde_json::from_str(&body).unwrap_or_default();
    let present = settings
        .index_sources
        .iter()
        .any(|source| source.id == BUILTIN_SOURCE_ID);
    if !present && !settings.builtin_removed {
        settings.index_sources.insert(0, builtin_source());
    }
    settings
}

pub fn save(paths: &AppPaths, settings: &Settings) -> AppResult<()> {
    let path = paths.settings_file();
    let mut body = serde_json::to_string_pretty(settings)?;
    body.push('\n');
    write_atomic(&path, body.as_bytes())
}

/// Write through a sibling temp file so a crash cannot truncate settings.
pub fn write_atomic(path: &Path, body: &[u8]) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    fs::write(&temp, body)
        .map_err(|problem| AppError::io(format!("could not write {}", temp.display()), problem))?;
    fs::rename(&temp, path)
        .map_err(|problem| AppError::io(format!("could not write {}", path.display()), problem))
}

pub fn remember_project(settings: &mut Settings, dir: &Path, cart: &cartcore::Cart) {
    let path = dir.to_string_lossy().to_string();
    let entry = RecentProject {
        path: path.clone(),
        id: cart
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        title: cart
            .get("title")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        base: cart
            .get("base")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        opened_at: now_iso(),
    };
    settings
        .recent_projects
        .retain(|recent| recent.path != path);
    settings.recent_projects.insert(0, entry);
    settings.recent_projects.truncate(12);
}

pub fn now_iso() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    format_iso(now)
}

/// Seconds since the epoch as an ISO-8601 UTC timestamp.
pub fn format_iso(seconds: u64) -> String {
    let days = seconds / 86_400;
    let time = seconds % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month,
        day,
        time / 3600,
        (time % 3600) / 60,
        time % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}
