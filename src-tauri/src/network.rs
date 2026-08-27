//! Commands that reach the network: pin resolution, index feeds and the online
//! validation pass. Every one of them can be cancelled and every one degrades
//! to a note rather than a failure when an API is unreachable.

use crate::error::{AppError, AppResult};
use crate::options::OptionDiscovery;
use crate::settings::IndexSourceSetting;
use crate::state::AppState;
use cartcore::index::IndexSource;
use resolve::{
    archive, gamebanana, github, ArchiveCache, CancelFlag, Client, DownloadOpts, FeedCache, GbPin,
    HttpError,
};
use serde::Serialize;
use serde_json::{Map, Value};
use std::path::Path;
use std::sync::Arc;

/// Manifest and schema files are small; nothing bigger is read out of a mod zip.
const MAX_MANIFEST_BYTES: u64 = 512 * 1024;
const MAX_LUA_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;

fn http_error(problem: HttpError) -> AppError {
    match problem {
        HttpError::NotFound(message) => AppError::not_found(message),
        other => AppError::network(other.to_string()),
    }
}

pub fn client(state: &AppState) -> Client {
    Client::new(state.github_token().as_deref())
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Resolution {
    #[serde(rename_all = "camelCase")]
    Pin { pin: Value, note: String },
    #[serde(rename_all = "camelCase")]
    ChooseFile {
        mod_id: u64,
        files: Vec<gamebanana::GbFile>,
    },
    #[serde(rename_all = "camelCase")]
    ChooseRelease {
        slug: String,
        releases: Vec<github::ReleaseSummary>,
    },
}

/// One of cartkit's pin specs, or the choice the user still has to make.
pub fn resolve_spec(
    state: &AppState,
    spec: &str,
    mod_id: Option<&str>,
    file_id: Option<u64>,
) -> AppResult<Resolution> {
    let client = client(state);
    let options = Map::new();
    match cartcore::parse_spec(spec) {
        Some(cartcore::Spec::Github { slug, version }) => {
            if !cartcore::spec::is_semver(&version) {
                return Err(AppError::invalid(format!(
                    "{} is not semver; pin an exact release like owner/repo@1.2.3",
                    version
                )));
            }
            let (pin, note) = github::pin_github(&client, &slug, &version, mod_id, &options)
                .map_err(http_error)?;
            Ok(Resolution::Pin { pin, note })
        }
        Some(cartcore::Spec::GameBanana { mod_id: gb_mod }) => {
            match gamebanana::pin_gamebanana(&client, gb_mod, file_id, mod_id, &options)
                .map_err(http_error)?
            {
                GbPin::Pinned { entry, note } => Ok(Resolution::Pin { pin: entry, note }),
                GbPin::Choose { mod_id, files } => Ok(Resolution::ChooseFile { mod_id, files }),
            }
        }
        None => {
            let slug = cartcore::spec::parse_slug(spec).ok_or_else(|| {
                AppError::invalid(format!(
                    "cannot read {}; write owner/repo@1.2.3, a github URL, a gamebanana mod url, or a gamebanana mod id",
                    spec
                ))
            })?;
            let releases = github::releases(&client, &slug).map_err(http_error)?;
            if releases.is_empty() {
                return Err(AppError::not_found(format!(
                    "{} publishes no releases; a cart can only pin a published build",
                    slug
                )));
            }
            Ok(Resolution::ChooseRelease { slug, releases })
        }
    }
}

pub fn github_releases(state: &AppState, slug: &str) -> AppResult<Vec<github::ReleaseSummary>> {
    let slug = cartcore::spec::parse_slug(slug)
        .ok_or_else(|| AppError::invalid("a repo looks like owner/name"))?;
    github::releases(&client(state), &slug).map_err(http_error)
}

pub fn gamebanana_files(state: &AppState, mod_id: u64) -> AppResult<Vec<gamebanana::GbFile>> {
    gamebanana::gamebanana_files(&client(state), mod_id).map_err(http_error)
}

pub fn validate_online(
    state: &AppState,
    dir: &Path,
    download: bool,
) -> AppResult<cartcore::Report> {
    let cart =
        cartcore::read_cart(dir).map_err(|problem| AppError::invalid(problem.to_string()))?;
    let cancel: CancelFlag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    Ok(resolve::validate_online(
        &cart,
        Some(dir),
        download,
        state.github_token().as_deref(),
        &cancel,
    ))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexFeed {
    pub source_id: String,
    pub label: String,
    pub fetched_at: String,
    pub stale: bool,
    pub from_fallback: bool,
    pub from_cache: bool,
    pub mods: Vec<cartcore::index::ModEntry>,
    pub carts: Vec<cartcore::index::CartEntry>,
    pub categories: Vec<String>,
    pub base_games: Vec<String>,
}

fn source_of(setting: &IndexSourceSetting) -> IndexSource {
    IndexSource {
        feed: setting.feed.clone(),
        base: setting.base.clone(),
        fallback: setting.fallback.clone(),
        label: setting.label.clone(),
    }
}

pub fn fetch_index(state: &AppState, source_id: &str, refresh: bool) -> AppResult<IndexFeed> {
    let settings = state.settings();
    let setting = settings
        .index_sources
        .iter()
        .find(|source| source.id == source_id)
        .ok_or_else(|| AppError::not_found("that index source is not configured"))?;
    let cache = FeedCache::new(state.paths.feeds.clone());
    let fetched = cache
        .load_source(&client(state), source_of(setting), refresh, false)
        .map_err(|problem| AppError::network(problem.to_string()))?;
    Ok(IndexFeed {
        source_id: source_id.to_string(),
        label: fetched.source.label.clone(),
        fetched_at: crate::settings::format_iso(fetched.fetched_at.max(0) as u64),
        stale: fetched.stale,
        from_fallback: fetched.from_fallback,
        from_cache: fetched.from_cache,
        mods: fetched.index.mods,
        carts: fetched.index.carts,
        categories: fetched.index.categories,
        base_games: fetched.index.base_games,
    })
}

/// Thumbnails are best effort: a failure returns an error the list can ignore.
pub fn fetch_thumbnail(state: &AppState, url: &str) -> AppResult<String> {
    let cache = FeedCache::new(state.paths.feeds.clone());
    let thumbnail = cache
        .thumbnail(&client(state), url)
        .map_err(|problem| AppError::network(problem.to_string()))?;
    use base64::Engine as _;
    Ok(format!(
        "data:{};base64,{}",
        thumbnail.content_type,
        base64::engine::general_purpose::STANDARD.encode(&thumbnail.bytes)
    ))
}

fn archive_for_pin(state: &AppState, pin: &Map<String, Value>) -> AppResult<std::path::PathBuf> {
    let cache = ArchiveCache::new(state.paths.archives.clone());
    let client = client(state);
    let cancel: CancelFlag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    match pin.get("source").and_then(Value::as_str) {
        Some("github") => {
            let sha256 = pin
                .get("sha256")
                .and_then(Value::as_str)
                .ok_or_else(|| AppError::invalid("that pin carries no sha256 yet"))?;
            let path = cache.path_for(sha256);
            if path.is_file() && archive::verify(&path, Some(sha256), None).is_ok() {
                return Ok(path);
            }
            let slug = pin
                .get("repo")
                .and_then(Value::as_str)
                .ok_or_else(|| AppError::invalid("that pin carries no repo"))?;
            let version = pin
                .get("version")
                .and_then(Value::as_str)
                .ok_or_else(|| AppError::invalid("that pin carries no version"))?;
            let id = pin.get("id").and_then(Value::as_str).unwrap_or_default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let opts = DownloadOpts {
                dest: Some(&path),
                max_bytes: Some(MAX_ARCHIVE_BYTES),
                cancel: Some(&cancel),
                progress: None,
                authed: false,
            };
            github::fetch_asset(&client, slug, version, id, &opts).map_err(http_error)?;
            archive::verify(&path, Some(sha256), None)
                .map_err(|problem| AppError::invalid(problem.to_string()))?;
            Ok(path)
        }
        Some("gamebanana") => Err(AppError::invalid(
            "GameBanana archives are not laid out as engine mods, so their options cannot be read; enter the values by hand",
        )),
        _ => Err(AppError::invalid("that pin has no resolvable source")),
    }
}

/// A mod's options, from its manifest schema when it has one and from running
/// its entry when it does not.
///
/// Four of the indexed mods publish `options_schema`; the rest register at
/// runtime, so the probe is the path that answers for almost everyone.
pub fn options_from_archive(state: &AppState, pin: Value) -> AppResult<OptionDiscovery> {
    let pin = match pin {
        Value::Object(map) => map,
        _ => return Err(AppError::invalid("a pin must be a JSON object")),
    };
    let path = archive_for_pin(state, &pin)?;
    let manifest_bytes = archive::read_entry(&path, "manifest.json", MAX_MANIFEST_BYTES)
        .map_err(|problem| AppError::invalid(problem.to_string()))?;
    let manifest_text = String::from_utf8_lossy(&manifest_bytes).to_string();
    let manifest =
        cartcore::modmanifest::parse_manifest(&manifest_text).map_err(AppError::invalid)?;

    if let Some(schema_path) = manifest.manifest.options_schema.clone() {
        match archive::read_entry(&path, &schema_path, MAX_MANIFEST_BYTES) {
            Ok(bytes) => {
                let source = String::from_utf8_lossy(&bytes).to_string();
                match cartcore::optionschema::evaluate_lua_schema(&source) {
                    Ok(rows) => {
                        return Ok(OptionDiscovery {
                            rows,
                            source: "archive",
                            error: None,
                        })
                    }
                    Err(problem) => return Ok(OptionDiscovery::none(Some(problem))),
                }
            }
            Err(problem) => return Ok(OptionDiscovery::none(Some(problem.to_string()))),
        }
    }

    probe_archive(
        &path,
        &manifest.manifest.entry,
        &manifest.manifest.id,
        &manifest.manifest.version,
    )
}

/// Run the mod's entry in cartcore's sandbox and keep what it registers.
fn probe_archive(
    path: &std::path::Path,
    entry: &str,
    id: &str,
    version: &str,
) -> AppResult<OptionDiscovery> {
    let names = match archive::list_entries(path) {
        Ok(names) => names,
        Err(problem) => return Ok(OptionDiscovery::none(Some(problem.to_string()))),
    };
    let mut sources = cartcore::optionprobe::Sources::new();
    for name in names {
        if let Ok(bytes) = archive::read_entry(path, &name, MAX_LUA_BYTES) {
            sources.insert(name, String::from_utf8_lossy(&bytes).to_string());
        }
    }
    if sources.is_empty() {
        return Ok(OptionDiscovery::none(Some(
            "this mod publishes no options_schema and carries no Lua to read".to_string(),
        )));
    }
    match cartcore::optionprobe::probe_entry(&sources, entry, id, version) {
        Ok(probe) => Ok(OptionDiscovery {
            rows: probe.rows,
            source: "probe",
            error: probe.note,
        }),
        Err(problem) => Ok(OptionDiscovery::none(Some(problem))),
    }
}

/// The engine's latest published release, for the scaffolded `engine` range.
pub fn latest_engine_version(state: &AppState) -> AppResult<String> {
    let client = client(state);
    let url = format!(
        "{}/repos/{}/releases/latest",
        client.api_base(),
        crate::settings::ENGINE_REPO
    );
    let payload = client
        .get_json(&url, Some("application/vnd.github+json"), true)
        .map_err(http_error)?;
    let tag = payload
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::network("that release names no tag"))?;
    let version = tag.trim_start_matches(['v', 'V']).to_string();
    if !cartcore::spec::is_semver(&version) {
        return Err(AppError::network(format!(
            "the engine's latest release is tagged {}, which is not a version",
            tag
        )));
    }
    Ok(version)
}
