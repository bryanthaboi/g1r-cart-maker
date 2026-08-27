//! A mod's `manifest.json`, with the vocabulary, defaults and range grammar of
//! src/mods/Manifest.lua, plus the dependency and conflict analysis over a set of
//! pinned mods. Untrusted input: unknown fields are tolerated, nothing is executed.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::schema::{BASES, MOD_PERMISSIONS, MOD_PROFILES};
use crate::semver;

/// GameVersion.ORDER, and the generation each id belongs to.
pub fn generation_of(id: &str) -> u32 {
    match id {
        "gold" | "silver" | "crystal" => 2,
        _ => 1,
    }
}

const MAX_IMPORT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencySpec {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub games: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModImport {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub file: String,
    pub md5: Vec<String>,
    pub format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_size: Option<u64>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub entry: String,
    pub api: u64,
    pub priority: f64,
    pub category: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
    pub profile: String,
    pub permissions: Vec<String>,
    pub experimental: bool,
    pub language: bool,
    pub games: Vec<String>,
    pub gen2compat: bool,
    pub affects_link: bool,
    pub dependencies: Vec<DependencySpec>,
    pub optional_dependencies: Vec<DependencySpec>,
    pub conflicts: Vec<DependencySpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_schema: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets_transforms: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_enable_env: Option<String>,
    pub required_imports: Vec<ModImport>,
    pub optional_imports: Vec<ModImport>,
}

/// A validated manifest plus the vocabulary violations api 1 downgrades to warnings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedManifest {
    pub manifest: ModManifest,
    pub warnings: Vec<String>,
}

// ------- helpers ported from the engine

/// SafePath.safe: a mod names files inside its own directory and nowhere else.
pub fn safe_path(rel: &str) -> Option<String> {
    if rel.is_empty() || rel.starts_with('/') || rel.contains('\\') {
        return None;
    }
    let bytes = rel.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return None;
    }
    let mut parts = Vec::new();
    for segment in rel.split('/').filter(|s| !s.is_empty()) {
        if segment == ".." {
            return None;
        }
        if segment != "." {
            parts.push(segment);
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

/// `owner/repo` or a github.com URL; empty or absent means no updates.
pub fn parse_github(value: &str) -> Result<Option<String>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let body = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .unwrap_or(trimmed);
    let body = body.strip_suffix('/').unwrap_or(body);
    let body = body.strip_suffix(".git").unwrap_or(body);
    let mut parts = body.split('/');
    let owner = parts.next().unwrap_or("");
    let repo = parts.next().unwrap_or("");
    if parts.next().is_some() || owner.is_empty() || repo.is_empty() {
        return Err("github must be owner/repo or a github.com URL".to_string());
    }
    let ok = |s: &str| {
        s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    };
    if !ok(owner) || !ok(repo) {
        return Err("github must be owner/repo or a github.com URL".to_string());
    }
    Ok(Some(format!("{}/{}", owner, repo)))
}

fn is_id(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn strip_bom(text: &str) -> String {
    text.strip_prefix('\u{feff}').unwrap_or(text).to_string()
}

/// ModTargets.expand: "red", "gen1" or "all".
fn expand_game(token: &str) -> Option<Vec<String>> {
    let key = token.trim().to_lowercase();
    if key == "all" {
        return Some(BASES.iter().map(|id| id.to_string()).collect());
    }
    if BASES.contains(&key.as_str()) {
        return Some(vec![key]);
    }
    let digits = key.strip_prefix("gen")?.trim();
    let generation: u32 = digits.parse().ok()?;
    let list = generation_versions(generation);
    if list.is_empty() {
        None
    } else {
        Some(list)
    }
}

fn generation_versions(generation: u32) -> Vec<String> {
    BASES
        .iter()
        .filter(|id| generation_of(id) == generation)
        .map(|id| id.to_string())
        .collect()
}

fn ordered(set: &BTreeSet<String>) -> Vec<String> {
    BASES
        .iter()
        .filter(|id| set.contains(**id))
        .map(|id| id.to_string())
        .collect()
}

fn normalize_games(list: &[Value]) -> (Vec<String>, Vec<String>) {
    let mut set = BTreeSet::new();
    let mut unknown = Vec::new();
    for token in list {
        match token.as_str().and_then(expand_game) {
            Some(ids) => set.extend(ids),
            None => unknown.push(value_label(token)),
        }
    }
    (ordered(&set), unknown)
}

fn value_label(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn array<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a [Value], String> {
    match value {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(list)) => Ok(list),
        Some(_) => Err(format!("{} must be an array", field)),
    }
}

// ------- dependency specs

/// Every shape the engine accepts: "id", "id@range", "id@range#repo", "id#repo",
/// {id, range|version, github|repo}, or a map of id -> range.
pub fn parse_dependency_specs(
    value: Option<&Value>,
    field: &str,
    sources: Option<&Value>,
) -> Result<Vec<DependencySpec>, String> {
    let entries: Vec<Value> = match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(list)) => list.clone(),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(id, range)| {
                let mut entry = serde_json::Map::new();
                entry.insert("id".to_string(), Value::String(id.clone()));
                if !range.is_null() {
                    entry.insert("range".to_string(), range.clone());
                }
                Value::Object(entry)
            })
            .collect(),
        Some(_) => return Err(format!("{} must be an array or an object", field)),
    };

    let mut out = Vec::new();
    for entry in &entries {
        out.push(parse_dependency_spec(entry, field, sources)?);
    }
    Ok(out)
}

fn parse_dependency_spec(
    entry: &Value,
    field: &str,
    sources: Option<&Value>,
) -> Result<DependencySpec, String> {
    let malformed = || format!("malformed {} entry {}", field, entry);
    let (id, range, mut github, games_raw, game_version) = match entry {
        Value::Object(map) => {
            let id = map
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(malformed)?
                .to_string();
            let range = map
                .get("range")
                .or_else(|| map.get("version"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let github = map
                .get("github")
                .or_else(|| map.get("repo"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let games = map.get("games").or_else(|| map.get("game")).cloned();
            let game_version = map
                .get("game_version")
                .and_then(Value::as_str)
                .map(str::to_string);
            (id, range, github, games, game_version)
        }
        Value::String(text) if !text.is_empty() => {
            let (main, repo) = match text.split_once('#') {
                Some((main, repo)) => (main, Some(repo.to_string())),
                None => (text.as_str(), None),
            };
            let (id, range) = match main.split_once('@') {
                Some((id, range)) if !range.is_empty() => (id.to_string(), Some(range.to_string())),
                Some(_) => return Err(malformed()),
                None => (main.to_string(), None),
            };
            (id, range, repo, None, None)
        }
        _ => {
            return Err(format!(
                "{} entries must be non-empty strings or objects",
                field
            ))
        }
    };

    if !is_id(&id) {
        return Err(malformed());
    }
    if let Some(range) = &range {
        semver::valid_range(range)
            .map_err(|err| format!("malformed {} range in {:?}: {}", field, id, err))?;
    }
    if let Some(game_version) = &game_version {
        semver::valid_range(game_version)
            .map_err(|err| format!("malformed {} game_version in {:?}: {}", field, id, err))?;
    }

    let games = match games_raw {
        None | Some(Value::Null) => None,
        Some(Value::String(token)) => {
            let (list, unknown) = normalize_games(&[Value::String(token)]);
            if !unknown.is_empty() {
                return Err(format!("unknown game in {}: {}", field, unknown.join(", ")));
            }
            Some(list)
        }
        Some(Value::Array(list)) => {
            let (list, unknown) = normalize_games(&list);
            if !unknown.is_empty() {
                return Err(format!("unknown game in {}: {}", field, unknown.join(", ")));
            }
            Some(list)
        }
        Some(_) => return Err("dependency games must be a string or an array".to_string()),
    };

    if github.is_none() {
        github = sources
            .and_then(Value::as_object)
            .and_then(|map| map.get(&id))
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    // a malformed hint is dropped, exactly as the engine's pcall does
    let github = github.and_then(|hint| parse_github(&hint).ok().flatten());

    Ok(DependencySpec {
        id,
        range,
        github,
        games,
        game_version,
    })
}

// ------- manifest

pub fn parse_manifest(json_text: &str) -> Result<ParsedManifest, String> {
    let raw: Value = serde_json::from_str(json_text)
        .map_err(|e| format!("manifest.json is not valid JSON: {}", e))?;
    validate_manifest(&raw)
}

pub fn validate_manifest(raw: &Value) -> Result<ParsedManifest, String> {
    let obj = raw
        .as_object()
        .ok_or_else(|| "manifest must be an object".to_string())?;
    let mut warnings = Vec::new();

    let id = obj
        .get("id")
        .and_then(Value::as_str)
        .map(strip_bom)
        .filter(|id| is_id(id))
        .ok_or_else(|| "manifest id must contain only letters, numbers, _ or -".to_string())?;
    let name = obj
        .get("name")
        .or_else(|| obj.get("title"))
        .and_then(Value::as_str)
        .map(strip_bom)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "manifest name is required".to_string())?;
    let version = obj
        .get("version")
        .and_then(Value::as_str)
        .map(strip_bom)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "manifest version is required".to_string())?;
    let entry_raw = obj
        .get("entry")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "manifest entry is required".to_string())?;
    let entry = safe_path(entry_raw).ok_or_else(|| {
        format!(
            "manifest entry must stay inside its root, got {:?}",
            entry_raw
        )
    })?;

    let api = match obj.get("api") {
        None | Some(Value::Null) => 1,
        Some(Value::Number(n)) => {
            let n = n.as_f64().unwrap_or(f64::NAN);
            if !n.is_finite() || n < 1.0 || n.fract() != 0.0 {
                return Err("manifest api must be a positive integer".to_string());
            }
            n as u64
        }
        Some(Value::String(text)) => text
            .trim()
            .parse::<u64>()
            .map_err(|_| "manifest api must be a number".to_string())?,
        Some(_) => return Err("manifest api must be a number".to_string()),
    };
    let strict = api >= 2;
    let violation = |warnings: &mut Vec<String>, message: String| -> Result<(), String> {
        if strict {
            return Err(message);
        }
        warnings.push(message);
        Ok(())
    };

    let mut profile = obj
        .get("profile")
        .and_then(Value::as_str)
        .unwrap_or("content")
        .to_string();
    if !MOD_PROFILES.contains(&profile.as_str()) {
        violation(&mut warnings, format!("unknown profile {:?}", profile))?;
        profile = "content".to_string();
    }

    let mut permissions = Vec::new();
    for entry in array(obj.get("permissions"), "permissions")? {
        match entry.as_str() {
            Some(name) if MOD_PERMISSIONS.contains(&name) => {
                if !permissions.iter().any(|held| held == name) {
                    permissions.push(name.to_string());
                }
            }
            _ => violation(
                &mut warnings,
                format!("unknown permission {:?}", value_label(entry)),
            )?,
        }
    }

    let game_version = match obj.get("game_version") {
        None | Some(Value::Null) => None,
        Some(Value::String(range)) => {
            semver::valid_range(range)
                .map_err(|err| format!("malformed game_version {:?}: {}", range, err))?;
            Some(range.clone())
        }
        Some(other) => return Err(format!("malformed game_version {}", other)),
    };

    let github = match obj.get("github") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => parse_github(text)?,
        Some(_) => return Err("github must be a string".to_string()),
    };

    let experimental = boolean(obj.get("experimental"), "experimental")?;
    let language = boolean(obj.get("language"), "language")?;
    let gen2_declared = boolean(obj.get("gen2compat"), "gen2compat")?;

    let games_raw = array(obj.get("games"), "games")?;
    let (mut games, unknown_games) = normalize_games(games_raw);
    for token in unknown_games {
        violation(&mut warnings, format!("unknown game {:?}", token))?;
    }
    if obj.get("games").is_some() && !obj["games"].is_null() && games.is_empty() {
        violation(
            &mut warnings,
            "games names no game this engine knows".to_string(),
        )?;
    }
    if games.is_empty() {
        games = if gen2_declared {
            BASES.iter().map(|id| id.to_string()).collect()
        } else {
            generation_versions(1)
        };
    } else if gen2_declared {
        let mut set: BTreeSet<String> = games.iter().cloned().collect();
        set.extend(generation_versions(2));
        games = ordered(&set);
    }
    let gen2compat = games.iter().any(|id| generation_of(id) == 2);

    let mut affects_link = profile != "content" && !language;
    if let Some(explicit) = obj.get("affects_link") {
        match explicit {
            Value::Bool(flag) => affects_link = *flag,
            Value::Null => {}
            _ => return Err("affects_link must be a boolean".to_string()),
        }
    }

    let log_url = match obj.get("log_url") {
        None | Some(Value::Null) => None,
        Some(value) if strict => {
            if !permissions.iter().any(|p| p == "network") {
                return Err("log_url requires the network permission".to_string());
            }
            match value.as_str() {
                Some(url) if url.starts_with("https://") => Some(url.to_string()),
                _ => return Err("log_url must be an https:// URL".to_string()),
            }
        }
        Some(_) => None,
    };

    let sources = obj.get("dependency_sources");
    let dependencies = parse_dependency_specs(obj.get("dependencies"), "dependencies", sources)?;
    let optional_dependencies = parse_dependency_specs(
        obj.get("optional_dependencies"),
        "optional_dependencies",
        sources,
    )?;

    let mut conflict_entries: Vec<Value> = Vec::new();
    for field in ["conflicts", "incompatible"] {
        for entry in array(obj.get(field), field)? {
            if !conflict_entries.contains(entry) {
                conflict_entries.push(entry.clone());
            }
        }
    }
    let conflicts =
        parse_dependency_specs(Some(&Value::Array(conflict_entries)), "conflicts", None)?;

    let required_imports = parse_imports(obj.get("required_imports"), "required_imports", true)?;
    let optional_imports = parse_imports(obj.get("optional_imports"), "optional_imports", false)?;
    let mut seen_ids = BTreeSet::new();
    let mut seen_files = BTreeSet::new();
    for import in required_imports.iter().chain(optional_imports.iter()) {
        if !seen_ids.insert(import.id.clone()) {
            return Err(format!("duplicate import id: {}", import.id));
        }
        if !seen_files.insert(import.file.clone()) {
            return Err(format!("duplicate import file: {}", import.file));
        }
    }

    Ok(ParsedManifest {
        manifest: ModManifest {
            id,
            name,
            version,
            entry,
            api,
            priority: obj
                .get("priority")
                .and_then(Value::as_f64)
                .filter(|n| n.is_finite())
                .unwrap_or(0.0),
            category: obj
                .get("category")
                .and_then(Value::as_str)
                .map(strip_bom)
                .unwrap_or_else(|| "OTHER".to_string()),
            description: obj
                .get("description")
                .and_then(Value::as_str)
                .map(strip_bom)
                .unwrap_or_default(),
            game_version,
            github,
            profile,
            permissions,
            experimental,
            language,
            games,
            gen2compat,
            affects_link,
            dependencies,
            optional_dependencies,
            conflicts,
            options_schema: optional_file(obj.get("options_schema"), "options_schema")?,
            assets_transforms: optional_file(obj.get("assets_transforms"), "assets_transforms")?,
            log_url,
            force_enable_env: optional_string(obj.get("force_enable_env"), "force_enable_env")?,
            required_imports,
            optional_imports,
        },
        warnings,
    })
}

fn boolean(value: Option<&Value>, field: &str) -> Result<bool, String> {
    match value {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(flag)) => Ok(*flag),
        Some(_) => Err(format!("{} must be a boolean", field)),
    }
}

fn optional_string(value: Option<&Value>, field: &str) -> Result<Option<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if !text.is_empty() => Ok(Some(text.clone())),
        Some(_) => Err(format!("{} must be a file path", field)),
    }
}

fn optional_file(value: Option<&Value>, field: &str) -> Result<Option<String>, String> {
    match optional_string(value, field)? {
        None => Ok(None),
        Some(text) => match safe_path(&text) {
            Some(path) => Ok(Some(path)),
            None => Err(format!(
                "{} must stay inside its root, got {:?}",
                field, text
            )),
        },
    }
}

fn parse_imports(
    value: Option<&Value>,
    field: &str,
    required: bool,
) -> Result<Vec<ModImport>, String> {
    let mut out: Vec<ModImport> = Vec::new();
    for entry in array(value, field)? {
        let map = entry
            .as_object()
            .ok_or_else(|| format!("{} entries must be objects", field))?;
        let id = map
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| is_id(id))
            .ok_or_else(|| format!("{} id must contain only letters, numbers, _ or -", field))?
            .to_string();
        if out.iter().any(|held| held.id == id) {
            return Err(format!("duplicate {} id: {}", field, id));
        }

        let file_raw = map
            .get("file")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("{} file is required", field))?;
        let file = safe_path(file_raw)
            .ok_or_else(|| format!("{} file must stay inside its root", field))?;
        if file.contains('/') {
            return Err(format!("{} file must be a filename inside baseroms", field));
        }
        if file.starts_with('.') {
            return Err(format!(
                "{} file must not use a hidden metadata filename",
                field
            ));
        }
        if out.iter().any(|held| held.file == file) {
            return Err(format!("duplicate {} file: {}", field, file));
        }

        let digests: Vec<&Value> = match map.get("md5") {
            Some(Value::String(_)) => vec![map.get("md5").unwrap()],
            Some(Value::Array(list)) if !list.is_empty() => list.iter().collect(),
            _ => return Err(format!("{} md5 must be a hash or a non-empty array", field)),
        };
        let mut md5 = Vec::new();
        for digest in digests {
            let text = digest
                .as_str()
                .filter(|d| d.len() == 32 && d.chars().all(|c| c.is_ascii_hexdigit()))
                .ok_or_else(|| format!("{} md5 values must be 32 hex characters", field))?
                .to_lowercase();
            if !md5.contains(&text) {
                md5.push(text);
            }
        }

        let format = map
            .get("format")
            .and_then(Value::as_str)
            .unwrap_or("raw")
            .to_string();
        if format != "raw" && format != "n64" {
            return Err(format!("{} format must be raw or n64", field));
        }
        let name = map
            .get("name")
            .and_then(Value::as_str)
            .map(strip_bom)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| id.clone());
        let description = map
            .get("description")
            .or_else(|| map.get("hint"))
            .and_then(Value::as_str)
            .map(strip_bom)
            .filter(|s| !s.is_empty());

        let size = import_size(map.get("size"), field, "size")?;
        let max_size = import_size(map.get("max_size"), field, "max_size")?;
        if let (Some(size), Some(max)) = (size, max_size) {
            if size > max {
                return Err(format!("{} size must not exceed max_size", field));
            }
        }

        out.push(ModImport {
            id,
            name,
            description,
            file,
            md5,
            format,
            size,
            max_size,
            required,
        });
    }
    Ok(out)
}

fn import_size(value: Option<&Value>, field: &str, label: &str) -> Result<Option<u64>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => {
            let raw = n.as_f64().unwrap_or(f64::NAN);
            if !raw.is_finite() || raw <= 0.0 || raw.fract() != 0.0 {
                return Err(format!("{} {} must be a positive integer", field, label));
            }
            let size = raw as u64;
            if size > MAX_IMPORT_BYTES {
                return Err(format!("{} {} exceeds the 2 GiB hard limit", field, label));
            }
            Ok(Some(size))
        }
        Some(_) => Err(format!("{} {} must be a positive integer", field, label)),
    }
}

// ------- dependency and conflict analysis

/// One pinned mod, as `mods[]` carries it plus the manifest's declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedMod {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub dependencies: Vec<DependencySpec>,
    #[serde(default)]
    pub optional_dependencies: Vec<DependencySpec>,
    #[serde(default)]
    pub conflicts: Vec<DependencySpec>,
}

impl PinnedMod {
    pub fn from_manifest(manifest: &ModManifest) -> PinnedMod {
        PinnedMod {
            id: manifest.id.clone(),
            version: manifest.version.clone(),
            dependencies: manifest.dependencies.clone(),
            optional_dependencies: manifest.optional_dependencies.clone(),
            conflicts: manifest.conflicts.clone(),
        }
    }
}

/// Only what the pinned manifests prove; asset collisions are not in this data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Issue {
    MissingDependency {
        mod_id: String,
        dependency: String,
        range: Option<String>,
    },
    UnsatisfiedDependency {
        mod_id: String,
        dependency: String,
        range: String,
        pinned: String,
        optional: bool,
    },
    CircularDependency {
        cycle: Vec<String>,
    },
    Conflict {
        mod_id: String,
        conflicts_with: String,
        range: Option<String>,
    },
}

impl Issue {
    pub fn message(&self) -> String {
        match self {
            Issue::MissingDependency {
                mod_id,
                dependency,
                range,
            } => match range {
                Some(range) => format!(
                    "{} needs {}@{}, which is not pinned",
                    mod_id, dependency, range
                ),
                None => format!("{} needs {}, which is not pinned", mod_id, dependency),
            },
            Issue::UnsatisfiedDependency {
                mod_id,
                dependency,
                range,
                pinned,
                optional,
            } => format!(
                "{} needs {}@{} but {} is pinned at {}{}",
                mod_id,
                dependency,
                range,
                dependency,
                pinned,
                if *optional { " (optional)" } else { "" }
            ),
            Issue::CircularDependency { cycle } => {
                format!("circular dependency: {}", cycle.join(" -> "))
            }
            Issue::Conflict {
                mod_id,
                conflicts_with,
                range,
            } => match range {
                Some(range) => format!(
                    "{} declares a conflict with {}@{}, which is pinned",
                    mod_id, conflicts_with, range
                ),
                None => format!(
                    "{} declares a conflict with {}, which is pinned",
                    mod_id, conflicts_with
                ),
            },
        }
    }
}

pub fn analyze(pins: &[PinnedMod]) -> Vec<Issue> {
    let by_id: HashMap<&str, &PinnedMod> = pins.iter().map(|pin| (pin.id.as_str(), pin)).collect();
    let mut order: Vec<&PinnedMod> = pins.iter().collect();
    order.sort_by(|a, b| a.id.cmp(&b.id));
    let mut issues = Vec::new();

    for pin in &order {
        for spec in &pin.dependencies {
            match by_id.get(spec.id.as_str()) {
                None => issues.push(Issue::MissingDependency {
                    mod_id: pin.id.clone(),
                    dependency: spec.id.clone(),
                    range: spec.range.clone(),
                }),
                Some(target) => check_range(&mut issues, pin, spec, target, false),
            }
        }
        for spec in &pin.optional_dependencies {
            if let Some(target) = by_id.get(spec.id.as_str()) {
                check_range(&mut issues, pin, spec, target, true);
            }
        }
    }

    for cycle in cycles(&order, &by_id) {
        issues.push(Issue::CircularDependency { cycle });
    }

    let mut seen_pairs = BTreeSet::new();
    for pin in &order {
        for spec in &pin.conflicts {
            let target = match by_id.get(spec.id.as_str()) {
                Some(target) if target.id != pin.id => target,
                _ => continue,
            };
            if let Some(range) = &spec.range {
                if !semver::satisfies(&target.version, range) {
                    continue;
                }
            }
            let pair = if pin.id < target.id {
                (pin.id.clone(), target.id.clone())
            } else {
                (target.id.clone(), pin.id.clone())
            };
            if !seen_pairs.insert(pair) {
                continue;
            }
            issues.push(Issue::Conflict {
                mod_id: pin.id.clone(),
                conflicts_with: spec.id.clone(),
                range: spec.range.clone(),
            });
        }
    }

    issues
}

fn check_range(
    issues: &mut Vec<Issue>,
    pin: &PinnedMod,
    spec: &DependencySpec,
    target: &PinnedMod,
    optional: bool,
) {
    let range = match &spec.range {
        Some(range) if !range.is_empty() => range,
        _ => return,
    };
    if semver::satisfies(&target.version, range) {
        return;
    }
    issues.push(Issue::UnsatisfiedDependency {
        mod_id: pin.id.clone(),
        dependency: spec.id.clone(),
        range: range.clone(),
        pinned: target.version.clone(),
        optional,
    });
}

/// Every distinct required-dependency cycle among the pins, each named once.
fn cycles(order: &[&PinnedMod], by_id: &HashMap<&str, &PinnedMod>) -> Vec<Vec<String>> {
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for pin in order {
        let mut targets: Vec<String> = Vec::new();
        for spec in &pin.dependencies {
            if by_id.contains_key(spec.id.as_str())
                && spec.id != pin.id
                && !targets.contains(&spec.id)
            {
                targets.push(spec.id.clone());
            }
        }
        targets.sort();
        edges.insert(pin.id.clone(), targets);
    }

    let mut found: BTreeSet<Vec<String>> = BTreeSet::new();
    let mut stack: Vec<String> = Vec::new();
    let mut on_stack: BTreeSet<String> = BTreeSet::new();
    let mut done: BTreeSet<String> = BTreeSet::new();
    for pin in order {
        walk(
            &pin.id,
            &edges,
            &mut stack,
            &mut on_stack,
            &mut done,
            &mut found,
        );
    }
    found.into_iter().collect()
}

fn walk(
    id: &str,
    edges: &BTreeMap<String, Vec<String>>,
    stack: &mut Vec<String>,
    on_stack: &mut BTreeSet<String>,
    done: &mut BTreeSet<String>,
    found: &mut BTreeSet<Vec<String>>,
) {
    if on_stack.contains(id) {
        let start = stack.iter().position(|held| held == id).unwrap_or(0);
        found.insert(normalize_cycle(&stack[start..]));
        return;
    }
    if done.contains(id) {
        return;
    }
    stack.push(id.to_string());
    on_stack.insert(id.to_string());
    for next in edges.get(id).map(Vec::as_slice).unwrap_or(&[]) {
        walk(next, edges, stack, on_stack, done, found);
    }
    on_stack.remove(id);
    stack.pop();
    done.insert(id.to_string());
}

/// Rotate to the lowest id so one cycle is reported once, however it was entered.
fn normalize_cycle(chain: &[String]) -> Vec<String> {
    if chain.is_empty() {
        return Vec::new();
    }
    let pivot = chain
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.cmp(b.1))
        .map(|(index, _)| index)
        .unwrap_or(0);
    let mut out: Vec<String> = chain[pivot..].to_vec();
    out.extend_from_slice(&chain[..pivot]);
    out.push(out[0].clone());
    out
}
