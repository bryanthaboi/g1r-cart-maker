//! The pure half of `src/mods/ModIndex.lua`: no network, no filesystem.
//! Serde names are the feed's own keys, so index.json maps unchanged.

use crate::semver::{lua_tonumber, satisfies};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// A feed is rebuilt on every push and refreshed nightly.
pub const CACHE_TTL: i64 = 24 * 60 * 60;
pub const SCHEMA_VERSION: i64 = 1;
pub const CACHE_VERSION: i64 = 3;

fn regex(cell: &'static OnceLock<regex::Regex>, pattern: &str) -> &'static regex::Regex {
    cell.get_or_init(|| regex::Regex::new(pattern).expect("static pattern"))
}

// ------- pure: source resolution

/// The four shapes of the same index resolve to one source. `base` always
/// keeps its trailing slash so `join_url` stays a concatenation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexSource {
    pub feed: String,
    pub base: String,
    pub fallback: Option<String>,
    pub label: String,
}

fn github_slug(url: &str) -> Option<(String, String)> {
    static REPO: OnceLock<regex::Regex> = OnceLock::new();
    static SLUG: OnceLock<regex::Regex> = OnceLock::new();
    let caps = regex(
        &REPO,
        r"^https?://github\.com/([0-9A-Za-z._-]+)/([0-9A-Za-z._-]+)",
    )
    .captures(url)
    .or_else(|| regex(&SLUG, r"^([0-9A-Za-z._-]+)/([0-9A-Za-z._-]+)$").captures(url))?;
    let owner = caps[1].to_string();
    let repo = caps[2].trim_end_matches(".git").to_string();
    Some((owner, repo))
}

pub fn resolve_source(input: &str) -> Result<IndexSource, String> {
    let url = input.trim();
    if url.is_empty() {
        return Err("missing index URL".to_string());
    }

    if let Some((owner, repo)) = github_slug(url) {
        return Ok(IndexSource {
            feed: format!("https://{}.github.io/{}/data/index.json", owner, repo),
            base: format!("https://{}.github.io/{}/", owner, repo),
            fallback: Some(format!(
                "https://raw.githubusercontent.com/{}/{}/main/site/data/index.json",
                owner, repo
            )),
            label: format!("{}/{}", owner, repo),
        });
    }

    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("index must be an http(s) URL or owner/repo".to_string());
    }

    // A feed URL names the file; the Pages root is what is left once the
    // "data/index.json" tail comes off (any other .json keeps only its folder).
    if url.ends_with(".json") {
        let base = url
            .strip_suffix("data/index.json")
            .filter(|head| head.ends_with('/'))
            .map(str::to_string)
            .or_else(|| url.rfind('/').map(|at| url[..=at].to_string()))
            .unwrap_or_else(|| url.to_string());
        return Ok(IndexSource {
            feed: url.to_string(),
            label: label_for(&base),
            base,
            fallback: None,
        });
    }

    let base = if url.ends_with('/') {
        url.to_string()
    } else {
        format!("{}/", url)
    };
    Ok(IndexSource {
        feed: format!("{}data/index.json", base),
        label: label_for(&base),
        base,
        fallback: None,
    })
}

/// A short human name for a source row. Only ever cosmetic.
pub fn label_for(url: &str) -> String {
    static PAGES: OnceLock<regex::Regex> = OnceLock::new();
    static HOST: OnceLock<regex::Regex> = OnceLock::new();
    let pages = regex(
        &PAGES,
        r"^https?://([0-9A-Za-z._-]+)\.github\.io/([0-9A-Za-z._-]+)/",
    );
    if let Some(caps) = pages.captures(url) {
        return format!("{}/{}", &caps[1], &caps[2]);
    }
    if let Some(caps) = regex(&HOST, r"^https?://([^/]+)/([^/]*)").captures(url) {
        if !caps[2].is_empty() {
            return format!("{}/{}", &caps[1], &caps[2]);
        }
        return caps[1].to_string();
    }
    url.to_string()
}

/// Relative feed paths resolve against the Pages root; an absolute one is
/// handed back untouched. Anything else is absent rather than an error.
pub fn join_url(base: &str, rel: &str) -> Option<String> {
    if rel.is_empty() {
        return None;
    }
    if rel.starts_with("http://") || rel.starts_with("https://") {
        return Some(rel.to_string());
    }
    if base.is_empty() {
        return None;
    }
    let base = if base.ends_with('/') {
        base.to_string()
    } else {
        format!("{}/", base)
    };
    Some(format!("{}{}", base, rel.trim_start_matches('/')))
}

// ------- pure: feed parsing

fn is_table(value: &Value) -> bool {
    value.is_object() || value.is_array()
}

fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.get(key).filter(|v| !v.is_null())
}

fn field_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    field(value, key).and_then(Value::as_str)
}

fn string_of(value: &Value, key: &str) -> Option<String> {
    field_str(value, key).map(str::to_string)
}

fn is_true(value: &Value, key: &str) -> bool {
    field(value, key) == Some(&Value::Bool(true))
}

fn number_of(value: &Value, key: &str) -> Option<f64> {
    match field(value, key)? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => lua_tonumber(s),
        _ => None,
    }
}

fn str_array(value: &Value, key: &str) -> Vec<String> {
    match field(value, key).and_then(Value::as_array) {
        Some(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        None => Vec::new(),
    }
}

fn num_array(value: &Value, key: &str) -> Vec<f64> {
    match field(value, key).and_then(Value::as_array) {
        Some(items) => items
            .iter()
            .filter_map(|item| match item {
                Value::Number(n) => n.as_f64(),
                Value::String(s) => lua_tonumber(s),
                _ => None,
            })
            .collect(),
        None => Vec::new(),
    }
}

/// `%d`-style formatting for the counts the warnings interpolate.
fn fmt_int(value: f64) -> String {
    format!("{}", value as i64)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Zip {
    pub name: Option<String>,
    pub url: String,
    pub size: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Latest {
    pub version: Option<String>,
    pub tag: Option<String>,
    pub name: Option<String>,
    pub prerelease: bool,
    pub published_at: Option<String>,
    pub zip: Option<Zip>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Downloads {
    pub total: Option<f64>,
    pub recent: Option<f64>,
    pub window_days: Option<f64>,
    pub as_of: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ReleaseDates {
    pub first: Option<String>,
    pub latest: Option<String>,
}

/// One release blob, in the shape `ModUpdate.parseRelease` produces.
fn parse_latest(raw: Option<&Value>) -> Option<Latest> {
    let raw = raw.filter(|v| is_table(v))?;
    let zip = field(raw, "zip").filter(|v| is_table(v)).and_then(|z| {
        Some(Zip {
            name: string_of(z, "name"),
            url: field_str(z, "url")?.to_string(),
            size: number_of(z, "size"),
        })
    });
    Some(Latest {
        version: string_of(raw, "version"),
        tag: string_of(raw, "tag"),
        name: string_of(raw, "name"),
        prerelease: is_true(raw, "prerelease"),
        published_at: string_of(raw, "published_at"),
        zip,
    })
}

fn parse_downloads(raw: Option<&Value>) -> Option<Downloads> {
    let raw = raw?;
    match raw {
        Value::Number(_) | Value::String(_) => {
            let total = match raw {
                Value::Number(n) => n.as_f64(),
                Value::String(s) => lua_tonumber(s),
                _ => None,
            };
            total.map(|total| Downloads {
                total: Some(total),
                ..Downloads::default()
            })
        }
        _ if !is_table(raw) => None,
        _ => {
            let out = Downloads {
                total: number_of(raw, "total"),
                recent: number_of(raw, "recent"),
                window_days: number_of(raw, "window_days"),
                as_of: string_of(raw, "as_of"),
            };
            if out.total.is_none() && out.recent.is_none() {
                return None;
            }
            Some(out)
        }
    }
}

pub fn download_stats<E: Entry + ?Sized>(entry: &E) -> Option<&Downloads> {
    entry.downloads()
}

fn iso_day(text: Option<&str>) -> Option<String> {
    static DAY: OnceLock<regex::Regex> = OnceLock::new();
    let caps = regex(&DAY, r"^(\d\d\d\d-\d\d-\d\d)").captures(text?)?;
    Some(caps[1].to_string())
}

pub fn release_dates<E: Entry + ?Sized>(entry: &E) -> Option<ReleaseDates> {
    let first = iso_day(entry.first_release());
    let latest = iso_day(entry.last_release())
        .or_else(|| iso_day(entry.latest().and_then(|l| l.published_at.as_deref())));
    if first.is_none() && latest.is_none() {
        return None;
    }
    Some(ReleaseDates { first, latest })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ModEntry {
    pub folder: Option<String>,
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub version: Option<String>,
    pub summary: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub games: Vec<String>,
    pub license: Option<String>,
    pub repo: Option<String>,
    pub github: Option<String>,
    #[serde(rename = "downloadURL")]
    pub download_url: Option<String>,
    pub api: Option<f64>,
    pub game_version: Option<String>,
    pub profile: Option<String>,
    pub affects_link: bool,
    pub experimental: bool,
    pub permissions: Vec<String>,
    pub dependencies: Option<Value>,
    pub conflicts: Option<Value>,
    pub thumbnail: Option<String>,
    pub description_url: Option<String>,
    pub downloads: Option<Downloads>,
    pub first_release: Option<String>,
    pub last_release: Option<String>,
    pub latest: Option<Latest>,
    pub update_check: String,
}

fn parse_entry(raw: &Value) -> Option<ModEntry> {
    if !is_table(raw) {
        return None;
    }
    let id = string_of(raw, "id")?;
    Some(ModEntry {
        folder: string_of(raw, "folder"),
        title: string_of(raw, "title").unwrap_or_else(|| id.clone()),
        id,
        author: string_of(raw, "author"),
        version: string_of(raw, "version"),
        summary: string_of(raw, "summary").unwrap_or_default(),
        categories: str_array(raw, "categories"),
        tags: str_array(raw, "tags"),
        games: str_array(raw, "games"),
        license: string_of(raw, "license"),
        repo: string_of(raw, "repo"),
        github: string_of(raw, "github"),
        download_url: string_of(raw, "downloadURL"),
        api: number_of(raw, "api"),
        game_version: string_of(raw, "game_version"),
        profile: string_of(raw, "profile"),
        affects_link: is_true(raw, "affects_link"),
        experimental: is_true(raw, "experimental"),
        permissions: str_array(raw, "permissions"),
        dependencies: field(raw, "dependencies").cloned(),
        conflicts: field(raw, "conflicts").cloned(),
        thumbnail: string_of(raw, "thumbnail"),
        description_url: string_of(raw, "description_url"),
        downloads: parse_downloads(field(raw, "downloads")),
        first_release: string_of(raw, "first_release"),
        last_release: string_of(raw, "last_release"),
        latest: parse_latest(field(raw, "latest")),
        update_check: string_of(raw, "update_check").unwrap_or_else(|| "pending".to_string()),
    })
}

/// github pins carry repo/version/sha256, gamebanana pins mod/file/md5.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CartPin {
    pub id: String,
    pub source: String,
    pub repo: Option<String>,
    pub version: Option<String>,
    pub sha256: Option<String>,
    #[serde(rename = "mod")]
    pub mod_id: Option<f64>,
    pub file: Option<f64>,
    pub md5: Option<String>,
    pub options: Option<Value>,
    pub enabled: Option<bool>,
}

fn parse_cart_pin(raw: &Value) -> Option<CartPin> {
    if !is_table(raw) {
        return None;
    }
    let id = string_of(raw, "id")?;
    let source = string_of(raw, "source")?;
    if source != "github" && source != "gamebanana" {
        return None;
    }
    Some(CartPin {
        id,
        source,
        repo: string_of(raw, "repo"),
        version: string_of(raw, "version"),
        sha256: string_of(raw, "sha256"),
        mod_id: number_of(raw, "mod"),
        file: number_of(raw, "file"),
        md5: string_of(raw, "md5"),
        options: field(raw, "options").filter(|v| is_table(v)).cloned(),
        enabled: match field(raw, "enabled") {
            Some(Value::Bool(false)) => Some(false),
            _ => None,
        },
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CartEntry {
    pub kind: String,
    pub folder: Option<String>,
    pub id: String,
    pub title: String,
    pub author: String,
    pub version: String,
    pub base: String,
    pub seal: String,
    pub summary: String,
    pub shell: Option<String>,
    pub finish: Option<String>,
    pub speeds: Vec<f64>,
    pub tags: Vec<String>,
    pub repo: String,
    pub github: Option<String>,
    #[serde(rename = "downloadURL")]
    pub download_url: Option<String>,
    pub automatic_version_check: bool,
    pub fixed_release_tag: Option<String>,
    pub game_version: Option<String>,
    pub license: Option<String>,
    pub mods: Vec<CartPin>,
    pub load_order: Vec<String>,
    pub thumbnail: Option<String>,
    pub description_url: Option<String>,
    pub downloads: Option<Downloads>,
    pub first_release: Option<String>,
    pub last_release: Option<String>,
    pub latest: Option<Latest>,
    pub update_check: String,
}

impl Default for CartEntry {
    fn default() -> Self {
        CartEntry {
            kind: "cart".to_string(),
            folder: None,
            id: String::new(),
            title: String::new(),
            author: String::new(),
            version: String::new(),
            base: String::new(),
            seal: String::new(),
            summary: String::new(),
            shell: None,
            finish: None,
            speeds: Vec::new(),
            tags: Vec::new(),
            repo: String::new(),
            github: None,
            download_url: None,
            automatic_version_check: true,
            fixed_release_tag: None,
            game_version: None,
            license: None,
            mods: Vec::new(),
            load_order: Vec::new(),
            thumbnail: None,
            description_url: None,
            downloads: None,
            first_release: None,
            last_release: None,
            latest: None,
            update_check: "pending".to_string(),
        }
    }
}

/// The eight required fields plus one valid pin are the gate: a row missing
/// any of them is dropped rather than half-listed.
fn parse_cart_entry(raw: &Value) -> Option<CartEntry> {
    if !is_table(raw) {
        return None;
    }
    let id = string_of(raw, "id")?;
    let title = string_of(raw, "title")?;
    let author = string_of(raw, "author")?;
    let version = string_of(raw, "version")?;
    let base = string_of(raw, "base")?;
    let seal = string_of(raw, "seal")?;
    let repo = string_of(raw, "repo")?;
    let raw_mods = field(raw, "mods").filter(|v| is_table(v))?;
    let pins: Vec<CartPin> = raw_mods
        .as_array()
        .map(|items| items.iter().filter_map(parse_cart_pin).collect())
        .unwrap_or_default();
    if pins.is_empty() {
        return None;
    }
    Some(CartEntry {
        kind: "cart".to_string(),
        folder: string_of(raw, "folder"),
        id,
        title,
        author,
        version,
        base,
        seal,
        summary: string_of(raw, "summary").unwrap_or_default(),
        shell: string_of(raw, "shell"),
        finish: string_of(raw, "finish"),
        speeds: num_array(raw, "speeds"),
        tags: str_array(raw, "tags"),
        repo,
        github: string_of(raw, "github"),
        download_url: string_of(raw, "downloadURL"),
        automatic_version_check: field(raw, "automatic_version_check") != Some(&Value::Bool(false)),
        fixed_release_tag: string_of(raw, "fixed_release_tag"),
        game_version: string_of(raw, "game_version"),
        license: string_of(raw, "license"),
        mods: pins,
        load_order: str_array(raw, "load_order"),
        thumbnail: string_of(raw, "thumbnail"),
        description_url: string_of(raw, "description_url"),
        downloads: parse_downloads(field(raw, "downloads")),
        first_release: string_of(raw, "first_release"),
        last_release: string_of(raw, "last_release"),
        latest: parse_latest(field(raw, "latest")),
        update_check: string_of(raw, "update_check").unwrap_or_else(|| "pending".to_string()),
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Index {
    pub schema_version: i64,
    pub generated_at: Option<String>,
    pub categories: Vec<String>,
    pub base_games: Vec<String>,
    pub mods: Vec<ModEntry>,
    pub carts: Vec<CartEntry>,
}

/// Never throws: a truncated download, an HTML error page, or a feed from a
/// future schema all come back as a message the panel can print.
pub fn parse_feed(json_text: &str) -> Result<Index, String> {
    let doc: Value = serde_json::from_str(json_text)
        .map_err(|err| format!("could not read the index: {}", err))?;
    if !doc.is_object() {
        return Err("index.json is not an object".to_string());
    }
    let schema = match number_of(&doc, "schema_version") {
        Some(schema) => schema,
        None => return Err("index.json has no schema_version".to_string()),
    };
    if schema as i64 != SCHEMA_VERSION {
        return Err(format!(
            "index schema {} is not supported (this build reads {})",
            fmt_int(schema),
            SCHEMA_VERSION
        ));
    }
    let raw_mods = match field(&doc, "mods").filter(|v| is_table(v)) {
        Some(raw_mods) => raw_mods,
        None => return Err("index.json has no mods array".to_string()),
    };
    let mods = raw_mods
        .as_array()
        .map(|items| items.iter().filter_map(parse_entry).collect())
        .unwrap_or_default();
    // Absent carts is the old-feed case, not an error.
    let carts = field(&doc, "carts")
        .filter(|v| is_table(v))
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(parse_cart_entry).collect())
        .unwrap_or_default();
    Ok(Index {
        schema_version: schema as i64,
        generated_at: string_of(&doc, "generated_at"),
        categories: str_array(&doc, "categories"),
        base_games: str_array(&doc, "base_games"),
        mods,
        carts,
    })
}

// ------- pure: the fields install resolution, compatibility and search read

/// Mod and cart listings are duck-typed in the Lua; this is that shared view.
pub trait Entry {
    fn id(&self) -> &str;
    fn title(&self) -> &str;
    fn author(&self) -> &str;
    fn summary(&self) -> &str;
    fn version(&self) -> Option<&str>;
    fn download_url(&self) -> Option<&str>;
    fn update_check(&self) -> &str;
    fn latest(&self) -> Option<&Latest>;
    fn tags(&self) -> &[String];
    fn downloads(&self) -> Option<&Downloads>;
    fn first_release(&self) -> Option<&str>;
    fn last_release(&self) -> Option<&str>;
    fn game_version(&self) -> Option<&str>;
    fn categories(&self) -> &[String] {
        &[]
    }
    fn base(&self) -> Option<&str> {
        None
    }
    fn api(&self) -> Option<f64> {
        None
    }
    fn profile(&self) -> Option<&str> {
        None
    }
    fn affects_link(&self) -> bool {
        false
    }
    fn experimental(&self) -> bool {
        false
    }
    fn permissions(&self) -> &[String] {
        &[]
    }
    fn dependencies(&self) -> Option<&Value> {
        None
    }
    fn conflicts(&self) -> Option<&Value> {
        None
    }
}

impl Entry for ModEntry {
    fn id(&self) -> &str {
        &self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn author(&self) -> &str {
        self.author.as_deref().unwrap_or("")
    }
    fn summary(&self) -> &str {
        &self.summary
    }
    fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
    fn download_url(&self) -> Option<&str> {
        self.download_url.as_deref()
    }
    fn update_check(&self) -> &str {
        &self.update_check
    }
    fn latest(&self) -> Option<&Latest> {
        self.latest.as_ref()
    }
    fn tags(&self) -> &[String] {
        &self.tags
    }
    fn downloads(&self) -> Option<&Downloads> {
        self.downloads.as_ref()
    }
    fn first_release(&self) -> Option<&str> {
        self.first_release.as_deref()
    }
    fn last_release(&self) -> Option<&str> {
        self.last_release.as_deref()
    }
    fn game_version(&self) -> Option<&str> {
        self.game_version.as_deref()
    }
    fn categories(&self) -> &[String] {
        &self.categories
    }
    fn api(&self) -> Option<f64> {
        self.api
    }
    fn profile(&self) -> Option<&str> {
        self.profile.as_deref()
    }
    fn affects_link(&self) -> bool {
        self.affects_link
    }
    fn experimental(&self) -> bool {
        self.experimental
    }
    fn permissions(&self) -> &[String] {
        &self.permissions
    }
    fn dependencies(&self) -> Option<&Value> {
        self.dependencies.as_ref()
    }
    fn conflicts(&self) -> Option<&Value> {
        self.conflicts.as_ref()
    }
}

impl Entry for CartEntry {
    fn id(&self) -> &str {
        &self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn author(&self) -> &str {
        &self.author
    }
    fn summary(&self) -> &str {
        &self.summary
    }
    fn version(&self) -> Option<&str> {
        Some(&self.version)
    }
    fn download_url(&self) -> Option<&str> {
        self.download_url.as_deref()
    }
    fn update_check(&self) -> &str {
        &self.update_check
    }
    fn latest(&self) -> Option<&Latest> {
        self.latest.as_ref()
    }
    fn tags(&self) -> &[String] {
        &self.tags
    }
    fn downloads(&self) -> Option<&Downloads> {
        self.downloads.as_ref()
    }
    fn first_release(&self) -> Option<&str> {
        self.first_release.as_deref()
    }
    fn last_release(&self) -> Option<&str> {
        self.last_release.as_deref()
    }
    fn game_version(&self) -> Option<&str> {
        self.game_version.as_deref()
    }
    fn base(&self) -> Option<&str> {
        Some(&self.base)
    }
}

// ------- pure: install resolution

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallKind {
    Release,
    Download,
}

/// A verified release asset first, then the author's fixed downloadURL. A
/// GitHub source-archive URL is never invented.
pub fn install_url<E: Entry + ?Sized>(entry: &E) -> Result<(String, InstallKind), String> {
    if entry.update_check() == "ok" {
        if let Some(url) = entry
            .latest()
            .and_then(|latest| latest.zip.as_ref())
            .map(|zip| zip.url.clone())
        {
            return Ok((url, InstallKind::Release));
        }
    }
    if let Some(url) = entry.download_url().filter(|url| !url.is_empty()) {
        return Ok((url.to_string(), InstallKind::Download));
    }
    match entry.update_check() {
        "off" => Err("the author does not publish installable releases".to_string()),
        "no installable release" => Err("no release with a .zip asset yet".to_string()),
        other if other.starts_with("error") => Err(other.to_string()),
        _ => Err("nothing installable listed".to_string()),
    }
}

pub fn can_install<E: Entry + ?Sized>(entry: &E) -> bool {
    install_url(entry).is_ok()
}

/// The release the index resolved when it could reach GitHub, else whatever
/// meta.json declared.
pub fn display_version<E: Entry + ?Sized>(entry: &E) -> String {
    if entry.update_check() == "ok" {
        if let Some(version) = entry.latest().and_then(|latest| latest.version.as_deref()) {
            return version.to_string();
        }
    }
    entry.version().unwrap_or("?").to_string()
}

/// A downloadURL entry has no release behind it, so one is synthesised around
/// the URL; the installer still validates the manifest inside.
pub fn release_for<E: Entry + ?Sized>(entry: &E) -> Result<Latest, String> {
    let (url, kind) = install_url(entry)?;
    if kind == InstallKind::Release {
        return entry
            .latest()
            .cloned()
            .ok_or_else(|| "nothing installable listed".to_string());
    }
    Ok(Latest {
        version: Some(display_version(entry)),
        zip: Some(Zip {
            name: Some(format!("{}.zip", entry.id())),
            url,
            size: None,
        }),
        ..Latest::default()
    })
}

// ------- pure: compatibility

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatIssue {
    pub level: String,
    pub text: String,
}

/// `installed` maps mod id to its version, as the launcher knows them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CompatContext {
    pub mod_api: Option<f64>,
    pub engine_version: Option<String>,
    pub installed: HashMap<String, String>,
}

/// dependencies / conflicts arrive as the manifest's own vocabulary: an array
/// of "id" / "id@range" / "id@range#repo" / {id=..}, or an id -> range map.
fn each_spec(spec: Option<&Value>, mut visit: impl FnMut(&str, Option<&str>)) {
    match spec {
        Some(Value::Array(items)) => {
            for item in items {
                match item {
                    Value::String(text) => {
                        let main = match text.split_once('#') {
                            Some((head, _)) if !head.is_empty() => head,
                            _ => text.as_str(),
                        };
                        match main.split_once('@') {
                            Some((id, range)) if !id.is_empty() && !range.is_empty() => {
                                visit(id, Some(range))
                            }
                            _ => visit(main, None),
                        }
                    }
                    _ if item.is_object() => {
                        if let Some(id) = field_str(item, "id") {
                            visit(id, field_str(item, "range").or(field_str(item, "version")));
                        }
                    }
                    _ => {}
                }
            }
        }
        Some(Value::Object(map)) => {
            for (id, value) in map {
                visit(id, value.as_str());
            }
        }
        _ => {}
    }
}

/// Soft gate by design: everything here warns and the player decides.
pub fn compat_issues<E: Entry + ?Sized>(entry: &E, ctx: &CompatContext) -> Vec<CompatIssue> {
    let mut out: Vec<CompatIssue> = Vec::new();
    let mut warn = |text: String| {
        out.push(CompatIssue {
            level: "warn".to_string(),
            text,
        })
    };

    if let (Some(api), Some(mod_api)) = (entry.api(), ctx.mod_api) {
        if api > mod_api {
            warn(format!(
                "Needs mod API {}; this build provides {}",
                fmt_int(api),
                fmt_int(mod_api)
            ));
        }
    }

    if let (Some(game_version), Some(engine)) =
        (entry.game_version(), ctx.engine_version.as_deref())
    {
        if !satisfies(engine, game_version) {
            warn(format!("Needs engine {} (have {})", game_version, engine));
        }
    }

    if let Some(profile) = entry.profile().filter(|profile| *profile != "content") {
        warn(format!(
            "Profile '{}' changes engine behaviour beyond content",
            profile
        ));
    }
    if entry.affects_link() {
        warn("Changes link play; both sides need the same mods".to_string());
    }
    if entry.experimental() {
        warn("Marked experimental by its author".to_string());
    }
    for name in entry.permissions() {
        warn(format!("Requests permission: {}", name));
    }

    each_spec(entry.dependencies(), |id, range| {
        if !ctx.installed.contains_key(id) {
            let range = range.map(|r| format!(" {}", r)).unwrap_or_default();
            warn(format!("Needs {}{} (not installed)", id, range));
        }
    });
    each_spec(entry.conflicts(), |id, _| {
        if ctx.installed.contains_key(id) {
            warn(format!("Conflicts with installed {}", id));
        }
    });

    out
}

// ------- pure: search / filter

fn haystack<E: Entry + ?Sized>(entry: &E) -> String {
    format!(
        "{} {} {} {}",
        entry.title(),
        entry.author(),
        entry.summary(),
        entry.id()
    )
    .to_lowercase()
}

/// Every whitespace-separated term must appear somewhere in title / author /
/// summary / id, so typing more narrows.
pub fn matches<E: Entry + ?Sized>(entry: &E, query: &str) -> bool {
    if query.trim().is_empty() {
        return true;
    }
    let hay = haystack(entry);
    query
        .trim()
        .to_lowercase()
        .split_whitespace()
        .all(|term| hay.contains(term))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FilterOpts {
    pub query: Option<String>,
    pub category: Option<String>,
    pub base: Option<String>,
    pub tag: Option<String>,
}

/// Category, base and tag compare case-insensitively; feed order is preserved.
pub fn filter<'a, E: Entry>(entries: &'a [E], opts: &FilterOpts) -> Vec<&'a E> {
    let want = opts.category.as_ref().map(|c| c.to_lowercase());
    let want_base = opts.base.as_ref().map(|b| b.to_lowercase());
    let want_tag = opts.tag.as_ref().map(|t| t.to_lowercase());
    let query = opts.query.as_deref().unwrap_or("");
    entries
        .iter()
        .filter(|entry| {
            if !matches(*entry, query) {
                return false;
            }
            if let Some(want) = &want {
                if !entry.categories().iter().any(|c| &c.to_lowercase() == want) {
                    return false;
                }
            }
            if let Some(want) = &want_base {
                if &entry.base().unwrap_or("").to_lowercase() != want {
                    return false;
                }
            }
            if let Some(want) = &want_tag {
                if !entry.tags().iter().any(|t| &t.to_lowercase() == want) {
                    return false;
                }
            }
            true
        })
        .collect()
}

/// Every category a feed actually uses, in its declared order, with anything
/// an entry names that the header forgot appended.
pub fn categories_in(index: &Index) -> Vec<String> {
    let used: Vec<&String> = index
        .mods
        .iter()
        .flat_map(|entry| entry.categories.iter())
        .collect();
    let mut out: Vec<String> = Vec::new();
    for category in &index.categories {
        if used.contains(&category) && !out.contains(category) {
            out.push(category.clone());
        }
    }
    for category in used {
        if !out.contains(category) {
            out.push(category.clone());
        }
    }
    out
}

/// The cart-side twin of `categories_in`.
pub fn base_games_in(index: &Index) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for base in &index.base_games {
        if index.carts.iter().any(|cart| &cart.base == base) && !out.contains(base) {
            out.push(base.clone());
        }
    }
    for cart in &index.carts {
        if !cart.base.is_empty() && !out.contains(&cart.base) {
            out.push(cart.base.clone());
        }
    }
    out
}

/// A cached listing is usable while it is this build's cache version and
/// younger than the TTL.
pub fn cache_fresh(checked_at: i64, version: i64, now: i64, ttl: i64) -> bool {
    version == CACHE_VERSION && (now - checked_at) < ttl
}

#[cfg(test)]
mod live {
    /// Smoke test against the real published feed. Ignored by default; run with
    /// `cargo test -p cartcore -- --ignored live_feed`.
    #[test]
    #[ignore]
    fn live_feed_parses() {
        let url = "https://bryanthaboi.github.io/gen1recomp-mod-index/data/index.json";
        let body = std::process::Command::new("curl")
            .args(["-fsSL", "--max-time", "30", url])
            .output()
            .expect("curl must run");
        assert!(body.status.success(), "the feed did not download");
        let text = String::from_utf8(body.stdout).expect("the feed must be utf-8");
        let index = super::parse_feed(&text).expect("the live feed must parse");
        assert!(index.mods.len() > 50, "got {} mods", index.mods.len());
        println!(
            "live feed: {} mods, {} carts, {} categories",
            index.mods.len(),
            index.carts.len(),
            index.categories.len()
        );
    }
}
