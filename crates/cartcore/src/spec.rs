//! Pin specs, frozen option values and derived ids, matching cartkit's parsers.

use crate::schema::{gamebanana_spec_re, github_slug_re, github_spec_re, semver_re};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Spec {
    /// `owner/repo@1.2.3`, with any leading v stripped from the version.
    Github { slug: String, version: String },
    /// A GameBanana mod id; the file is chosen separately.
    GameBanana { mod_id: u64 },
}

pub fn parse_spec(spec: &str) -> Option<Spec> {
    let spec = spec.trim();
    if let Some(caps) = github_spec_re().captures(spec) {
        return Some(Spec::Github {
            slug: format!("{}/{}", &caps[1], &caps[2]),
            version: caps[3].trim_start_matches(['v', 'V']).to_string(),
        });
    }
    if let Some(caps) = gamebanana_spec_re().captures(spec) {
        return Some(Spec::GameBanana {
            mod_id: caps[1].parse().ok()?,
        });
    }
    None
}

/// `owner/repo` or a github.com URL, with no version attached.
pub fn parse_slug(text: &str) -> Option<String> {
    github_slug_re()
        .captures(text.trim())
        .map(|caps| format!("{}/{}", &caps[1], &caps[2]))
}

pub fn is_semver(text: &str) -> bool {
    semver_re().is_match(text)
}

#[derive(Debug, thiserror::Error)]
pub enum OptionParseError {
    #[error("--option wants key=value (got {0})")]
    Shape(String),
    /// cartkit would write `Infinity` here, which is not JSON; refuse it instead.
    #[error("option {0} must be a finite number")]
    NotFinite(String),
}

/// `k=v`, where true/false and numbers keep their JSON type.
pub fn parse_option(text: &str) -> Result<(String, Value), OptionParseError> {
    let (key, raw) = match text.split_once('=') {
        Some((key, raw)) if !key.is_empty() => (key, raw),
        _ => {
            return Err(OptionParseError::Shape(crate::validate::python_repr(text)));
        }
    };
    let lowered = raw.trim().to_lowercase();
    if lowered == "true" || lowered == "false" {
        return Ok((key.to_string(), json!(lowered == "true")));
    }
    if let Ok(int) = raw.trim().parse::<i64>() {
        return Ok((key.to_string(), json!(int)));
    }
    if let Ok(float) = raw.trim().parse::<f64>() {
        if !float.is_finite() {
            return Err(OptionParseError::NotFinite(crate::validate::python_repr(
                key,
            )));
        }
        return Ok((key.to_string(), json!(float)));
    }
    Ok((key.to_string(), json!(raw)))
}

/// A mod id derived from a repo name or an archive filename.
pub fn derive_id(text: &str) -> String {
    let mut cleaned = String::new();
    let mut pending_dash = false;
    for ch in text.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            if pending_dash && !cleaned.is_empty() {
                cleaned.push('-');
            }
            pending_dash = false;
            cleaned.push(ch);
        } else {
            pending_dash = true;
        }
    }
    let cleaned = cleaned.trim_matches('-').to_string();
    let cleaned = if cleaned.is_empty() {
        "mod".to_string()
    } else {
        cleaned
    };
    cleaned.chars().take(64).collect::<String>().to_lowercase()
}

pub fn is_placeholder(entry: &serde_json::Map<String, Value>) -> bool {
    entry
        .get("repo")
        .and_then(Value::as_str)
        .map(|repo| repo.to_lowercase() == crate::schema::PLACEHOLDER_REPO)
        .unwrap_or(false)
        && entry.get("sha256").and_then(Value::as_str) == Some(crate::schema::PLACEHOLDER_SHA)
}
