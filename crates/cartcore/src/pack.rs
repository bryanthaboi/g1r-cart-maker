//! The `.g1rcart` bundle: a deterministic Lua table, label art embedded base64.

use crate::cart::Cart;
use crate::luaenc::{lua_encode, EncodeError};
use crate::schema::{BUNDLE_FORMAT, BUNDLE_VERSION, CART_KEYS, MOD_KEYS};
use base64::Engine as _;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

/// Only documented fields, schema dropped, seal and load_order materialized.
pub fn packed_cart(cart: &Cart) -> Map<String, Value> {
    let mut out = Map::new();
    for key in CART_KEYS {
        if let Some(value) = cart.get(key) {
            if !value.is_null() {
                out.insert(key.to_string(), value.clone());
            }
        }
    }
    out.remove("schema");
    let shell = cart
        .get("shell")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    out.insert("shell".into(), Value::String(shell));
    let seal = cart
        .get("seal")
        .cloned()
        .unwrap_or_else(|| Value::String("sealed".into()));
    out.insert("seal".into(), seal);

    let mut mods = Vec::new();
    if let Some(Value::Array(entries)) = cart.get("mods") {
        for entry in entries {
            let entry = match entry {
                Value::Object(map) => map,
                _ => continue,
            };
            let mut pin = Map::new();
            for key in MOD_KEYS {
                if let Some(value) = entry.get(key) {
                    if !value.is_null() {
                        pin.insert(key.to_string(), value.clone());
                    }
                }
            }
            let empty_options = match pin.get("options") {
                Some(Value::Object(map)) => map.is_empty(),
                Some(Value::Array(items)) => items.is_empty(),
                Some(Value::String(text)) => text.is_empty(),
                Some(Value::Bool(flag)) => !flag,
                Some(Value::Number(number)) => number.as_f64() == Some(0.0),
                _ => true,
            };
            if empty_options {
                pin.remove("options");
            }
            mods.push(Value::Object(pin));
        }
    }
    let order = match cart.get("load_order") {
        Some(Value::Array(items)) if !items.is_empty() => Value::Array(items.clone()),
        _ => Value::Array(
            mods.iter()
                .map(|entry| entry.get("id").cloned().unwrap_or(Value::Null))
                .collect(),
        ),
    };
    out.insert("mods".into(), Value::Array(mods));
    out.insert("load_order".into(), order);
    out
}

pub fn bundle_table(cart: &Cart, label_bytes: Option<&[u8]>, label_name: Option<&str>) -> Value {
    let mut root = Map::new();
    root.insert("format".into(), json!(BUNDLE_FORMAT));
    root.insert("formatVersion".into(), json!(BUNDLE_VERSION));
    root.insert("cart".into(), Value::Object(packed_cart(cart)));
    if let Some(bytes) = label_bytes.filter(|bytes| !bytes.is_empty()) {
        let mut art = Map::new();
        art.insert(
            "name".into(),
            label_name.map(|n| json!(n)).unwrap_or(Value::Null),
        );
        art.insert("encoding".into(), json!("base64"));
        art.insert("bytes".into(), json!(bytes.len()));
        art.insert(
            "data".into(),
            json!(base64::engine::general_purpose::STANDARD.encode(bytes)),
        );
        root.insert("labelArt".into(), Value::Object(art));
    }
    Value::Object(root)
}

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("{0}")]
    Encode(#[from] EncodeError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

pub fn bundle_bytes(cart: &Cart, cart_dir: Option<&Path>) -> Result<Vec<u8>, PackError> {
    let mut label_bytes = None;
    let mut label_name = None;
    if let (Some(label), Some(dir)) = (cart.get("label").and_then(Value::as_str), cart_dir) {
        let path = dir.join(label);
        if path.is_file() {
            label_bytes = Some(fs::read(&path)?);
            label_name = Path::new(label)
                .file_name()
                .map(|name| name.to_string_lossy().to_string());
        }
    }
    let table = bundle_table(cart, label_bytes.as_deref(), label_name.as_deref());
    Ok(lua_encode(&table)?.into_bytes())
}

/// cartkit's default output name for a packed cart.
pub fn bundle_name(cart: &Cart) -> String {
    let id = cart.get("id").and_then(Value::as_str).unwrap_or("cart");
    let version = cart
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("0.0.0");
    format!("{}-{}{}", id, version, crate::schema::CART_EXT)
}
