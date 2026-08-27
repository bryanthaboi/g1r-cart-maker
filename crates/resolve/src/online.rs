//! The online half of validation. A reachable API that says "no" is a finding;
//! an API that could not be reached is a note, so a rate limit never fails a
//! cart that is fine.

use crate::gamebanana::{gamebanana_file, gamebanana_files};
use crate::github::resolve_github_watched;
use crate::http::{github_token, CancelFlag, Client, DownloadOpts, HttpError};
use cartcore::findings::{err, Finding};
use cartcore::schema::{repo_re, semver_re};
use cartcore::Cart;
use serde_json::{Map, Value};
use std::sync::atomic::Ordering;

pub const NO_TOKEN_NOTE: &str =
    "not signed in to GitHub, so these calls are anonymous and capped at 60 an hour; \
     run gh auth login and press Re-check";

fn field<'a>(entry: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    entry.get(key).and_then(Value::as_str)
}

/// How a Python f-string renders a value that may be missing.
fn shown(entry: &Map<String, Value>, key: &str) -> String {
    match entry.get(key) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Null) | None => "None".to_string(),
        Some(Value::Bool(flag)) => if *flag { "True" } else { "False" }.to_string(),
        Some(other) => other.to_string(),
    }
}

/// A JSON integer that is not a bool and not a float, above zero.
fn positive_int(entry: &Map<String, Value>, key: &str) -> Option<u64> {
    match entry.get(key) {
        Some(Value::Number(number)) if number.is_i64() || number.is_u64() => number
            .as_i64()
            .filter(|value| *value > 0)
            .map(|value| value as u64),
        _ => None,
    }
}

pub fn online_findings(
    cart: &Cart,
    download: bool,
    token: Option<&str>,
    cancel: &CancelFlag,
) -> (Vec<Finding>, Vec<String>) {
    let client = Client::new(token);
    online_findings_with(&client, cart, download, cancel)
}

/// The same pass against a caller-owned client, so a UI can reuse one agent.
pub fn online_findings_with(
    client: &Client,
    cart: &Cart,
    download: bool,
    cancel: &CancelFlag,
) -> (Vec<Finding>, Vec<String>) {
    let mut findings = Vec::new();
    let mut notes = Vec::new();
    if github_token(client.token()).is_none() {
        notes.push(NO_TOKEN_NOTE.to_string());
    }
    for (index, item) in cartcore::cart::mods_of(cart).iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            notes.push("online validation cancelled".to_string());
            break;
        }
        let entry = match item.as_object() {
            Some(entry) => entry,
            None => continue,
        };
        let label = format!("mods[{}] {}", index + 1, shown(entry, "id"));
        match field(entry, "source") {
            Some("github") => github_pin(
                client,
                entry,
                &label,
                download,
                cancel,
                &mut findings,
                &mut notes,
            ),
            Some("gamebanana") => gamebanana_pin(client, entry, &label, &mut findings, &mut notes),
            _ => {}
        }
    }
    (findings, notes)
}

fn github_pin(
    client: &Client,
    entry: &Map<String, Value>,
    label: &str,
    download: bool,
    cancel: &CancelFlag,
    findings: &mut Vec<Finding>,
    notes: &mut Vec<String>,
) {
    let slug = field(entry, "repo").unwrap_or_default();
    let version = field(entry, "version").unwrap_or_default();
    if !repo_re().is_match(slug) || !semver_re().is_match(version) {
        return;
    }
    let opts = DownloadOpts {
        cancel: Some(cancel),
        ..DownloadOpts::default()
    };
    let mod_id = field(entry, "id").unwrap_or_default();
    let found = match resolve_github_watched(client, slug, version, mod_id, download, &opts) {
        Ok(found) => found,
        Err(HttpError::NotFound(problem)) => {
            findings.push(err(
                "CK100",
                format!("{} does not resolve: {}", label, problem),
            ));
            return;
        }
        Err(HttpError::Cancelled) => {
            notes.push(format!("{} not resolved: cancelled", label));
            return;
        }
        Err(problem) => {
            notes.push(format!("{} not resolved: {}", label, problem));
            return;
        }
    };
    let digest = match found.sha256 {
        Some(digest) => digest,
        None => {
            notes.push(format!(
                "{} hash not checked: the release publishes no sha256sums.txt and \
                 --no-download was given",
                label
            ));
            return;
        }
    };
    if Some(digest.as_str()) != field(entry, "sha256") {
        findings.push(err(
            "CK101",
            format!(
                "{} pins sha256 {} but {} on {} {} hashes to {}; re-pin it",
                label,
                shown(entry, "sha256"),
                found.asset,
                slug,
                found.tag,
                digest
            ),
        ));
    }
}

fn gamebanana_pin(
    client: &Client,
    entry: &Map<String, Value>,
    label: &str,
    findings: &mut Vec<Finding>,
    notes: &mut Vec<String>,
) {
    let (mod_id, file_id) = match (positive_int(entry, "mod"), positive_int(entry, "file")) {
        (Some(mod_id), Some(file_id)) => (mod_id, file_id),
        _ => return,
    };
    let files = match gamebanana_files(client, mod_id) {
        Ok(files) => files,
        Err(HttpError::NotFound(problem)) => {
            findings.push(err(
                "CK110",
                format!("{} does not resolve: {}", label, problem),
            ));
            return;
        }
        Err(problem) => {
            notes.push(format!("{} not resolved: {}", label, problem));
            return;
        }
    };
    let found = match gamebanana_file(&files, file_id) {
        Some(found) => found,
        None => {
            let have: Vec<String> = files.iter().map(|file| file.id_label()).collect();
            findings.push(err(
                "CK110",
                format!(
                    "{} pins file {}, which is not on GameBanana mod {} (it publishes {})",
                    label,
                    file_id,
                    mod_id,
                    have.join(", ")
                ),
            ));
            return;
        }
    };
    if Some(found.md5.as_str()) != field(entry, "md5") {
        let published = if found.md5.is_empty() {
            "no checksum".to_string()
        } else {
            found.md5.clone()
        };
        findings.push(err(
            "CK111",
            format!(
                "{} pins md5 {} but file {} ({}) publishes {}; re-pin it",
                label,
                shown(entry, "md5"),
                file_id,
                found.file,
                published
            ),
        ));
    }
}
