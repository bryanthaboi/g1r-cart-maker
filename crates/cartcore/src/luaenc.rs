//! Lua table encoder matching `cartkit.lua_encode` byte for byte.

use serde_json::Value;

/// `%.<precision>g` as C and Python format it.
pub fn format_g(value: f64, precision: usize) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0".to_string()
        } else {
            "0".to_string()
        };
    }
    let precision = precision.max(1);
    let sci = format!("{:.*e}", precision - 1, value);
    let (mantissa, exp) = sci.split_once('e').expect("rust exponential format");
    let exp: i32 = exp.parse().expect("rust exponent digits");
    if exp < -4 || exp >= precision as i32 {
        let mantissa = trim_zeros(mantissa);
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{}{:02}", mantissa, sign, exp.abs())
    } else {
        let decimals = (precision as i32 - 1 - exp).max(0) as usize;
        trim_zeros(&format!("{:.*}", decimals, value))
    }
}

fn trim_zeros(text: &str) -> String {
    if !text.contains('.') {
        return text.to_string();
    }
    let trimmed = text.trim_end_matches('0');
    trimmed.trim_end_matches('.').to_string()
}

/// Quote a string the way cartkit does: `"` `\` and newline escape literally,
/// other control characters become decimal escapes, padded when a digit follows.
pub fn lua_string(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for (position, ch) in chars.iter().enumerate() {
        let code = *ch as u32;
        if *ch == '"' || *ch == '\\' || *ch == '\n' {
            out.push('\\');
            out.push(*ch);
        } else if code < 32 || code == 127 {
            let following = chars.get(position + 1).copied();
            if following.map(|c| c.is_ascii_digit()).unwrap_or(false) {
                out.push_str(&format!("\\{:03}", code));
            } else {
                out.push_str(&format!("\\{}", code));
            }
        } else {
            out.push(*ch);
        }
    }
    out.push('"');
    out
}

#[derive(Debug, thiserror::Error)]
#[error("cannot serialize {0}")]
pub struct EncodeError(pub String);

pub fn lua_number(value: &serde_json::Number) -> Result<String, EncodeError> {
    if let Some(int) = value.as_i64() {
        return Ok(int.to_string());
    }
    if let Some(int) = value.as_u64() {
        return Ok(int.to_string());
    }
    let float = value
        .as_f64()
        .ok_or_else(|| EncodeError(value.to_string()))?;
    if !float.is_finite() {
        return Err(EncodeError(float.to_string()));
    }
    Ok(format_g(float, 14))
}

fn is_lua_name(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn lua_value(value: &Value, indent: usize) -> Result<String, EncodeError> {
    let pad = "  ".repeat(indent);
    match value {
        Value::Bool(flag) => Ok(if *flag { "true".into() } else { "false".into() }),
        Value::Number(number) => lua_number(number),
        Value::String(text) => Ok(lua_string(text)),
        Value::Array(items) => {
            if items.is_empty() {
                return Ok("{}".into());
            }
            let mut lines = Vec::with_capacity(items.len());
            for (position, item) in items.iter().enumerate() {
                lines.push(format!(
                    "{}  [{}] = {}",
                    pad,
                    position + 1,
                    lua_value(item, indent + 1)?
                ));
            }
            Ok(format!("{{\n{},\n{}}}", lines.join(",\n"), pad))
        }
        Value::Object(map) => {
            if map.is_empty() {
                return Ok("{}".into());
            }
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            let mut lines = Vec::with_capacity(keys.len());
            for key in keys {
                let rendered = if is_lua_name(key) {
                    key.to_string()
                } else {
                    format!("[{}]", lua_string(key))
                };
                lines.push(format!(
                    "{}  {} = {}",
                    pad,
                    rendered,
                    lua_value(&map[key], indent + 1)?
                ));
            }
            Ok(format!("{{\n{},\n{}}}", lines.join(",\n"), pad))
        }
        Value::Null => Err(EncodeError("null".into())),
    }
}

pub fn lua_encode(data: &Value) -> Result<String, EncodeError> {
    Ok(format!("return {}\n", lua_value(data, 0)?))
}
