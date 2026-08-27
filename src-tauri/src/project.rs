//! A project is a cart directory on disk. There is no wrapper project file.

use crate::error::{AppError, AppResult};
use crate::settings::Settings;
use base64::Engine as _;
use cartcore::cart::{read_cart, write_cart, Cart, CartError};
use cartcore::labeldoc::{parse_doc, serialize_doc, LabelDoc, DOC_FILE};
use cartcore::pack::{bundle_bytes, bundle_name};
use cartcore::scaffold::{scaffold_into, ScaffoldOptions};
use cartcore::schema::CART_FILE;
use cartcore::workflow::WORKFLOW_PATH;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelInfo {
    pub path: Option<String>,
    pub exists: bool,
    pub bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub data_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectState {
    pub dir: String,
    pub cart: Value,
    pub label: LabelInfo,
    pub label_doc: Option<LabelDoc>,
    pub report: cartcore::Report,
    pub has_workflow: bool,
    pub is_git_repo: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaffoldRequest {
    pub parent: String,
    pub id: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub base: String,
    pub shell: Option<String>,
    pub seal: String,
    pub github: Option<String>,
    #[serde(default)]
    pub force: bool,
}

fn cart_error(problem: CartError) -> AppError {
    match problem {
        CartError::Missing => AppError::not_found(format!(
            "no {} in that directory; pick a cart directory or create a new cart",
            CART_FILE
        )),
        other => AppError::invalid(other.to_string()),
    }
}

pub fn png_data_url(bytes: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// Accepts only a PNG data URL; the manifest draws nothing else.
pub fn decode_png_data_url(data_url: &str) -> AppResult<Vec<u8>> {
    let encoded = data_url
        .strip_prefix("data:image/png;base64,")
        .ok_or_else(|| AppError::invalid("label art must be a PNG data URL"))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|problem| {
            AppError::invalid(format!("label art is not valid base64: {}", problem))
        })?;
    if !cartcore::labelart::is_png(&bytes) {
        return Err(AppError::invalid("label art is not a PNG"));
    }
    Ok(bytes)
}

fn label_info(dir: &Path, cart: &Cart) -> LabelInfo {
    let label = cart
        .get("label")
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut info = LabelInfo {
        path: label.clone(),
        exists: false,
        bytes: 0,
        width: None,
        height: None,
        data_url: None,
    };
    let label = match label {
        Some(label) => label,
        None => return info,
    };
    if cartcore::validate::label_problem(&Value::String(label.clone())).is_some() {
        return info;
    }
    let path = dir.join(&label);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(_) => return info,
    };
    info.exists = true;
    info.bytes = bytes.len() as u64;
    if let Some((width, height)) = cartcore::labelart::png_dimensions(&bytes) {
        info.width = Some(width);
        info.height = Some(height);
    }
    info.data_url = Some(png_data_url(&bytes));
    info
}

pub fn read_label_doc(dir: &Path) -> Option<LabelDoc> {
    let body = fs::read_to_string(dir.join(DOC_FILE)).ok()?;
    parse_doc(&body).ok()
}

pub fn write_label_doc(dir: &Path, doc: &LabelDoc) -> AppResult<()> {
    crate::settings::write_atomic(&dir.join(DOC_FILE), serialize_doc(doc).as_bytes())
}

pub fn state(dir: &Path) -> AppResult<ProjectState> {
    let cart = read_cart(dir).map_err(cart_error)?;
    let report = cartcore::validate_cart(&cart, Some(dir));
    Ok(ProjectState {
        dir: dir.to_string_lossy().to_string(),
        label: label_info(dir, &cart),
        label_doc: read_label_doc(dir),
        report,
        has_workflow: dir.join(WORKFLOW_PATH).is_file(),
        is_git_repo: dir.join(".git").exists(),
        cart: Value::Object(cart),
    })
}

pub fn open(dir: &Path, settings: &mut Settings) -> AppResult<ProjectState> {
    let state = state(dir)?;
    if let Value::Object(cart) = &state.cart {
        crate::settings::remember_project(settings, dir, cart);
    }
    Ok(state)
}

pub fn scaffold(request: &ScaffoldRequest, settings: &mut Settings) -> AppResult<ProjectState> {
    let parent = PathBuf::from(&request.parent);
    if !parent.is_dir() {
        fs::create_dir_all(&parent).map_err(|problem| {
            AppError::io(format!("could not create {}", parent.display()), problem)
        })?;
    }
    let dest = parent.join(&request.id);
    let options = ScaffoldOptions {
        id: request.id.clone(),
        title: request.title.clone().filter(|text| !text.trim().is_empty()),
        author: request
            .author
            .clone()
            .filter(|text| !text.trim().is_empty()),
        summary: request
            .summary
            .clone()
            .filter(|text| !text.trim().is_empty()),
        base: request.base.clone(),
        shell: request.shell.clone().filter(|text| !text.trim().is_empty()),
        seal: request.seal.clone(),
        github: request
            .github
            .clone()
            .filter(|text| !text.trim().is_empty()),
        engine: settings.engine_version.clone(),
        force: request.force,
    };
    scaffold_into(&dest, &options).map_err(|problem| AppError::invalid(problem.to_string()))?;
    open(&dest, settings)
}

pub fn save(dir: &Path, cart: Value) -> AppResult<ProjectState> {
    let cart = match cart {
        Value::Object(map) => map,
        _ => return Err(AppError::invalid("a cart must be a JSON object")),
    };
    write_cart(dir, &cart)
        .map_err(|problem| AppError::io(format!("could not write {}", CART_FILE), problem))?;
    state(dir)
}

pub fn update_cart<F>(dir: &Path, edit: F) -> AppResult<ProjectState>
where
    F: FnOnce(&mut Cart) -> AppResult<()>,
{
    let mut cart = read_cart(dir).map_err(cart_error)?;
    edit(&mut cart)?;
    write_cart(dir, &cart)
        .map_err(|problem| AppError::io(format!("could not write {}", CART_FILE), problem))?;
    state(dir)
}

pub fn validate(dir: &Path) -> AppResult<cartcore::Report> {
    let cart = read_cart(dir).map_err(cart_error)?;
    Ok(cartcore::validate_cart(&cart, Some(dir)))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub bytes: u64,
}

/// Pack runs strict validation: a warning refuses the bundle, as cartkit does.
pub fn export_bundle(dir: &Path, out_path: &Path) -> AppResult<ExportResult> {
    let cart = read_cart(dir).map_err(cart_error)?;
    let findings = cartcore::schema_findings(&cart, Some(dir));
    if !findings.is_empty() {
        let detail = findings
            .iter()
            .map(|finding| finding.line())
            .collect::<Vec<_>>()
            .join("\n");
        return Err(AppError::invalid(
            "export refused: packing runs strict validation, so warnings are fatal too",
        )
        .with_detail(detail));
    }
    let body =
        bundle_bytes(&cart, Some(dir)).map_err(|problem| AppError::invalid(problem.to_string()))?;
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(out_path, &body).map_err(|problem| {
        AppError::io(format!("could not write {}", out_path.display()), problem)
    })?;
    Ok(ExportResult {
        path: out_path.to_string_lossy().to_string(),
        bytes: body.len() as u64,
    })
}

pub fn default_bundle_name(dir: &Path) -> AppResult<String> {
    let cart = read_cart(dir).map_err(cart_error)?;
    Ok(bundle_name(&cart))
}

fn mods_mut(cart: &mut Cart) -> &mut Vec<Value> {
    if !matches!(cart.get("mods"), Some(Value::Array(_))) {
        cart.insert("mods".into(), Value::Array(Vec::new()));
    }
    match cart.get_mut("mods") {
        Some(Value::Array(items)) => items,
        _ => unreachable!("mods was just set to an array"),
    }
}

/// A new pin replaces one with the same id and evicts the scaffold placeholder.
pub fn add_pin(dir: &Path, pin: Value) -> AppResult<ProjectState> {
    let pin = match pin {
        Value::Object(map) => map,
        _ => return Err(AppError::invalid("a pin must be a JSON object")),
    };
    let id = pin
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::invalid("a pin needs an id"))?
        .to_string();
    update_cart(dir, |cart| {
        let mods = mods_mut(cart);
        let existing = mods
            .iter()
            .position(|entry| entry.get("id").and_then(Value::as_str) == Some(id.as_str()));
        let mut dropped: Vec<String> = Vec::new();
        match existing {
            Some(index) => {
                let mut merged = pin.clone();
                if let Some(Value::Object(kept)) = mods[index].get("options") {
                    if !merged.contains_key("options") && !kept.is_empty() {
                        merged.insert("options".into(), Value::Object(kept.clone()));
                    }
                }
                mods[index] = Value::Object(merged);
            }
            None => {
                mods.retain(|entry| {
                    let placeholder = entry
                        .as_object()
                        .map(cartcore::spec::is_placeholder)
                        .unwrap_or(false);
                    if placeholder {
                        if let Some(id) = entry.get("id").and_then(Value::as_str) {
                            dropped.push(id.to_string());
                        }
                    }
                    !placeholder
                });
                mods.push(Value::Object(pin.clone()));
            }
        }
        if let Some(Value::Array(order)) = cart.get_mut("load_order") {
            order.retain(|entry| {
                entry
                    .as_str()
                    .map(|name| !dropped.iter().any(|gone| gone == name))
                    .unwrap_or(false)
            });
            if !order
                .iter()
                .any(|entry| entry.as_str() == Some(id.as_str()))
            {
                order.push(Value::String(id.clone()));
            }
        }
        Ok(())
    })
}

pub fn remove_pin(dir: &Path, id: &str) -> AppResult<ProjectState> {
    update_cart(dir, |cart| {
        mods_mut(cart).retain(|entry| entry.get("id").and_then(Value::as_str) != Some(id));
        if let Some(Value::Array(order)) = cart.get_mut("load_order") {
            order.retain(|entry| entry.as_str() != Some(id));
        }
        Ok(())
    })
}

pub fn reorder_pins(dir: &Path, order: Vec<String>) -> AppResult<ProjectState> {
    update_cart(dir, |cart| {
        let ids: Vec<String> = mods_mut(cart)
            .iter()
            .filter_map(|entry| entry.get("id").and_then(Value::as_str).map(str::to_string))
            .collect();
        for id in &order {
            if !ids.contains(id) {
                return Err(AppError::invalid(format!(
                    "load order names {}, which is not pinned",
                    id
                )));
            }
        }
        for id in &ids {
            if !order.contains(id) {
                return Err(AppError::invalid(format!(
                    "load order leaves out {}; name every pinned mod",
                    id
                )));
            }
        }
        cart.insert(
            "load_order".into(),
            Value::Array(order.into_iter().map(Value::String).collect()),
        );
        Ok(())
    })
}

pub fn set_pin_options(dir: &Path, id: &str, options: Value) -> AppResult<ProjectState> {
    let options = match options {
        Value::Object(map) => map,
        _ => return Err(AppError::invalid("options must be a JSON object")),
    };
    update_cart(dir, |cart| {
        let entry = mods_mut(cart)
            .iter_mut()
            .find(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
            .ok_or_else(|| AppError::not_found(format!("{} is not pinned", id)))?;
        let entry = entry
            .as_object_mut()
            .ok_or_else(|| AppError::invalid("that pin is not an object"))?;
        if options.is_empty() {
            entry.remove("options");
        } else {
            entry.insert("options".into(), Value::Object(options.clone()));
        }
        Ok(())
    })
}

pub fn set_pin_enabled(dir: &Path, id: &str, enabled: bool) -> AppResult<ProjectState> {
    update_cart(dir, |cart| {
        let entry = mods_mut(cart)
            .iter_mut()
            .find(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
            .ok_or_else(|| AppError::not_found(format!("{} is not pinned", id)))?;
        let entry = entry
            .as_object_mut()
            .ok_or_else(|| AppError::invalid("that pin is not an object"))?;
        if enabled {
            entry.remove("enabled");
        } else {
            entry.insert("enabled".into(), Value::Bool(false));
        }
        Ok(())
    })
}

/// The licences the app offers to write. Anything else is the author's own job.
pub fn license_text(spdx: &str, holder: &str) -> Option<String> {
    let year = "2026";
    let holder = if holder.trim().is_empty() {
        "the cart author"
    } else {
        holder.trim()
    };
    match spdx {
        "MIT" => Some(format!(
            include_str!("licenses/mit.txt"),
            year = year,
            holder = holder
        )),
        "CC0-1.0" => Some(include_str!("licenses/cc0.txt").to_string()),
        "Apache-2.0" => Some(format!(
            include_str!("licenses/apache.txt"),
            year = year,
            holder = holder
        )),
        _ => None,
    }
}
