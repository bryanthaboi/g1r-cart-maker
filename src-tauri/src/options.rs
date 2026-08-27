//! Mod option discovery. A snapshot is data; a schema chunk is untrusted code
//! and only ever runs in cartcore's sandbox.

use crate::error::{AppError, AppResult};
use cartcore::optionschema::{parse_snapshot, Row};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

pub const SNAPSHOT_FILE: &str = "mod_option_schemas.json";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionDiscovery {
    pub rows: Vec<Row>,
    pub source: &'static str,
    pub error: Option<String>,
}

impl OptionDiscovery {
    pub fn none(error: Option<String>) -> Self {
        Self {
            rows: Vec::new(),
            source: "none",
            error,
        }
    }
}

/// The engine's post-boot snapshot beside options.lua; execute nothing from it.
pub fn from_install(save_dir: &Path) -> AppResult<HashMap<String, OptionDiscovery>> {
    let path = save_dir.join(SNAPSHOT_FILE);
    let body = std::fs::read_to_string(&path).map_err(|problem| {
        AppError::not_found(format!(
            "no {} in {}; run the game once with the mods enabled",
            SNAPSHOT_FILE,
            save_dir.display()
        ))
        .with_detail(problem.to_string())
    })?;
    let snapshot = parse_snapshot(&body).map_err(AppError::invalid)?;
    Ok(snapshot
        .into_iter()
        .map(|(id, rows)| {
            (
                id,
                OptionDiscovery {
                    rows,
                    source: "install",
                    error: None,
                },
            )
        })
        .collect())
}
