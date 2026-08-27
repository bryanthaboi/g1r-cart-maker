//! cart.json as it sits on disk: a JSON object whose key order is preserved and
//! whose unknown fields survive a round trip.

use crate::schema::{CART_FILE, CART_KEYS};
use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub type Cart = Map<String, Value>;

#[derive(Debug, thiserror::Error)]
pub enum CartError {
    #[error("{CART_FILE} missing; run New Cart to make one")]
    Missing,
    #[error("{CART_FILE} unparseable: {0}")]
    Unparseable(String),
    #[error("{CART_FILE} must be a JSON object")]
    NotAnObject,
    #[error("{0}")]
    Io(#[from] io::Error),
}

pub fn cart_path(cart_dir: &Path) -> PathBuf {
    cart_dir.join(CART_FILE)
}

pub fn read_cart(cart_dir: &Path) -> Result<Cart, CartError> {
    let path = cart_path(cart_dir);
    if !path.is_file() {
        return Err(CartError::Missing);
    }
    let body = fs::read_to_string(&path)?;
    parse_cart(&body)
}

pub fn parse_cart(body: &str) -> Result<Cart, CartError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|problem| CartError::Unparseable(problem.to_string()))?;
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(CartError::NotAnObject),
    }
}

/// Documented keys first, in cartkit's order, then anything else the author kept.
pub fn ordered(cart: &Cart) -> Cart {
    let mut ordered = Map::new();
    for key in CART_KEYS {
        if let Some(value) = cart.get(key) {
            ordered.insert(key.to_string(), value.clone());
        }
    }
    for (key, value) in cart {
        if !ordered.contains_key(key) {
            ordered.insert(key.clone(), value.clone());
        }
    }
    ordered
}

pub fn serialize_cart(cart: &Cart) -> String {
    let mut body = serde_json::to_string_pretty(&Value::Object(ordered(cart)))
        .expect("a cart map always serializes");
    body.push('\n');
    body
}

pub fn write_cart(cart_dir: &Path, cart: &Cart) -> io::Result<()> {
    fs::create_dir_all(cart_dir)?;
    fs::write(cart_path(cart_dir), serialize_cart(cart))
}

pub fn cart_str<'a>(cart: &'a Cart, key: &str) -> Option<&'a str> {
    cart.get(key).and_then(Value::as_str)
}

pub fn mods_of(cart: &Cart) -> &[Value] {
    match cart.get("mods") {
        Some(Value::Array(items)) => items,
        _ => &[],
    }
}
