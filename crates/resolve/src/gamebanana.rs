//! GameBanana through the v11 API only, which is the one endpoint that
//! publishes a per-file md5 a cart can pin.

use crate::http::{Client, HttpError};
use cartcore::schema::md5_re;
use serde_json::{json, Map, Value};

/// One row of `_aFiles`. `id` stays optional so a malformed row still lists.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GbFile {
    pub id: Option<u64>,
    pub file: String,
    pub filesize: u64,
    pub md5: String,
    pub description: String,
    pub download_count: u64,
    pub download_url: Option<String>,
}

impl GbFile {
    /// How cartkit renders the id when it lists what a mod publishes.
    pub fn id_label(&self) -> String {
        match self.id {
            Some(id) => id.to_string(),
            None => "None".to_string(),
        }
    }
}

fn number(entry: &Map<String, Value>, key: &str) -> u64 {
    entry
        .get(key)
        .and_then(Value::as_u64)
        .or_else(|| {
            entry
                .get(key)
                .and_then(Value::as_str)
                .and_then(|text| text.parse().ok())
        })
        .unwrap_or(0)
}

fn text(entry: &Map<String, Value>, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn as_file(entry: &Map<String, Value>) -> GbFile {
    GbFile {
        id: entry.get("_idRow").and_then(Value::as_u64),
        file: text(entry, "_sFile"),
        filesize: number(entry, "_nFilesize"),
        md5: text(entry, "_sMd5Checksum").to_lowercase(),
        description: text(entry, "_sDescription"),
        download_count: number(entry, "_nDownloadCount"),
        download_url: entry
            .get("_sDownloadUrl")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

pub fn download_page_url(client: &Client, mod_id: u64) -> String {
    format!(
        "{}/apiv11/Mod/{}/DownloadPage",
        client.gamebanana_base(),
        mod_id
    )
}

/// Every file on a mod. A trashed, withheld or empty mod is a NotFound.
pub fn gamebanana_files(client: &Client, mod_id: u64) -> Result<Vec<GbFile>, HttpError> {
    let url = download_page_url(client, mod_id);
    let payload = client.get_json(&url, None, false)?;
    let payload = match payload.as_object() {
        Some(payload) => payload,
        None => {
            return Err(HttpError::Unreachable(format!(
                "{}: unexpected response",
                url
            )))
        }
    };
    let flagged = |key: &str| {
        payload
            .get(key)
            .map(|value| match value {
                Value::Bool(flag) => *flag,
                Value::Number(number) => number.as_f64().unwrap_or(0.0) != 0.0,
                Value::String(text) => !text.is_empty(),
                Value::Null => false,
                _ => true,
            })
            .unwrap_or(false)
    };
    if flagged("_bIsTrashed") || flagged("_bIsWithheld") {
        return Err(HttpError::NotFound(format!(
            "GameBanana mod {} is trashed or withheld",
            mod_id
        )));
    }
    let files = payload.get("_aFiles").and_then(Value::as_array);
    let files = match files {
        Some(files) if !files.is_empty() => files,
        _ => {
            return Err(HttpError::NotFound(format!(
                "GameBanana mod {} publishes no files",
                mod_id
            )))
        }
    };
    Ok(files
        .iter()
        .filter_map(Value::as_object)
        .map(as_file)
        .collect())
}

pub fn gamebanana_file(files: &[GbFile], file_id: u64) -> Option<&GbFile> {
    files.iter().find(|entry| entry.id == Some(file_id))
}

/// A pin, or the file list to choose from when the mod publishes more than one.
#[derive(Debug, Clone)]
pub enum GbPin {
    Pinned { entry: Value, note: String },
    Choose { mod_id: u64, files: Vec<GbFile> },
}

pub fn pin_gamebanana(
    client: &Client,
    mod_id: u64,
    file_id: Option<u64>,
    id: Option<&str>,
    options: &Map<String, Value>,
) -> Result<GbPin, HttpError> {
    let files = gamebanana_files(client, mod_id)?;
    let chosen = match file_id {
        Some(file_id) => match gamebanana_file(&files, file_id) {
            Some(chosen) => chosen.clone(),
            None => {
                let have: Vec<String> = files
                    .iter()
                    .map(|entry| format!("{} ({})", entry.id_label(), entry.file))
                    .collect();
                return Err(HttpError::NotFound(format!(
                    "GameBanana mod {} has no file {}; it publishes {}",
                    mod_id,
                    file_id,
                    have.join(", ")
                )));
            }
        },
        None if files.len() == 1 => files[0].clone(),
        None => return Ok(GbPin::Choose { mod_id, files }),
    };
    if !md5_re().is_match(&chosen.md5) {
        return Err(HttpError::NotFound(format!(
            "GameBanana file {} publishes no md5; a cart cannot pin it",
            chosen.id_label()
        )));
    }
    let chosen_id = match chosen.id {
        Some(id) => id,
        None => {
            return Err(HttpError::NotFound(format!(
                "GameBanana mod {} publishes a file with no id",
                mod_id
            )))
        }
    };
    let stem = match chosen.file.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => chosen.file.as_str(),
    };
    let name = match id {
        Some(id) => id.to_string(),
        None => {
            let derived = cartcore::spec::derive_id(stem);
            if derived.is_empty() {
                format!("gb{}", mod_id)
            } else {
                derived
            }
        }
    };
    let mut entry = Map::new();
    entry.insert("id".into(), json!(name));
    entry.insert("source".into(), json!("gamebanana"));
    entry.insert("mod".into(), json!(mod_id));
    entry.insert("file".into(), json!(chosen_id));
    entry.insert("md5".into(), json!(chosen.md5));
    if !options.is_empty() {
        entry.insert("options".into(), Value::Object(options.clone()));
    }
    let note = format!(
        "gamebanana {} -> file {} ({}), md5 from the v11 API",
        mod_id, chosen_id, chosen.file
    );
    Ok(GbPin::Pinned {
        entry: Value::Object(entry),
        note,
    })
}
