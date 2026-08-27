//! Mod option rows: the `mod_option_schemas.json` snapshot (docs/mod-option-schema.md)
//! and the sandboxed `options_schema` chunk, plus the coercion that freezes a value
//! into `mods[].options`. Everything parsed here is untrusted.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

use crate::schema::{MAX_OPTIONS, MAX_OPTION_KEY, MAX_OPTION_TEXT};

/// The only envelope version this consumer understands.
pub const SNAPSHOT_VERSION: u64 = 1;
pub const SNAPSHOT_FILE: &str = "mod_option_schemas.json";

/// Instruction budget for one `options_schema` chunk.
const STEP_LIMIT: u64 = 4_000_000;
const STEP_GRANULARITY: u32 = 10_000;
const MEMORY_LIMIT: usize = 16 * 1024 * 1024;
const MAX_LUA_DEPTH: usize = 16;
const MAX_LUA_NODES: usize = 50_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RowKind {
    Toggle {
        #[serde(default)]
        default: bool,
    },
    Choice {
        #[serde(default)]
        default: Value,
        #[serde(default)]
        choices: Vec<(String, Value)>,
    },
    Number {
        #[serde(default)]
        default: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        step: Option<f64>,
    },
    Text {
        #[serde(default)]
        default: String,
        #[serde(
            default,
            rename = "maxLen",
            skip_serializing_if = "Option::is_none",
            alias = "max_len"
        )]
        max_len: Option<usize>,
    },
}

/// `visible_if` with neither arm set is the engine's "always hidden" row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisibleIf {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_equals: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub key: String,
    pub label: String,
    #[serde(flatten)]
    pub kind: RowKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_if: Option<VisibleIf>,
}

impl Row {
    pub fn type_name(&self) -> &'static str {
        match self.kind {
            RowKind::Toggle { .. } => "toggle",
            RowKind::Choice { .. } => "choice",
            RowKind::Number { .. } => "number",
            RowKind::Text { .. } => "text",
        }
    }

    pub fn default_value(&self) -> Value {
        match &self.kind {
            RowKind::Toggle { default } => Value::Bool(*default),
            RowKind::Choice { default, .. } => default.clone(),
            RowKind::Number { default, .. } => number_value(*default),
            RowKind::Text { default, .. } => Value::String(default.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroppedOption {
    pub key: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frozen {
    pub options: Map<String, Value>,
    pub dropped: Vec<DroppedOption>,
}

// ------- snapshot

/// The engine's `mod_option_schemas.json`, keyed by mod id. A newer envelope
/// is refused rather than guessed at.
pub fn parse_snapshot(json_text: &str) -> Result<HashMap<String, Vec<Row>>, String> {
    let doc: Value = serde_json::from_str(json_text)
        .map_err(|e| format!("{} is not valid JSON: {}", SNAPSHOT_FILE, e))?;
    let doc = doc
        .as_object()
        .ok_or_else(|| format!("{} must be a JSON object", SNAPSHOT_FILE))?;

    let version = match doc.get("schema_version") {
        None | Some(Value::Null) => SNAPSHOT_VERSION,
        Some(Value::Number(n)) => n
            .as_u64()
            .ok_or_else(|| "schema_version must be a positive integer".to_string())?,
        Some(_) => return Err("schema_version must be a number".to_string()),
    };
    if version == 0 {
        return Err("schema_version must be a positive integer".to_string());
    }
    if version > SNAPSHOT_VERSION {
        return Err(format!(
            "{} is schema_version {}; this build reads version {}",
            SNAPSHOT_FILE, version, SNAPSHOT_VERSION
        ));
    }

    let mods = match doc.get("mods") {
        None | Some(Value::Null) => return Ok(HashMap::new()),
        Some(Value::Object(map)) => map,
        Some(_) => return Err("\"mods\" must be an object keyed by mod id".to_string()),
    };

    let mut out = HashMap::new();
    for (id, rows) in mods {
        if let Some(list) = rows.as_array() {
            out.insert(id.clone(), parse_rows(list));
        }
    }
    Ok(out)
}

/// Rows the engine would render; malformed and unknown-typed rows are skipped.
pub fn parse_rows(rows: &[Value]) -> Vec<Row> {
    rows.iter().filter_map(parse_row).collect()
}

fn parse_row(raw: &Value) -> Option<Row> {
    let obj = raw.as_object()?;
    let key = obj.get("key")?.as_str()?.to_string();
    if key.is_empty() {
        return None;
    }
    let label = obj
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or(key.as_str())
        .to_string();
    let kind = match obj.get("type")?.as_str()? {
        "toggle" => RowKind::Toggle {
            default: obj.get("default").and_then(Value::as_bool).unwrap_or(false),
        },
        "choice" => RowKind::Choice {
            default: obj.get("default").cloned().unwrap_or(Value::Null),
            choices: parse_choices(obj.get("choices")),
        },
        "number" => RowKind::Number {
            default: obj.get("default").and_then(finite).unwrap_or(0.0),
            min: obj.get("min").and_then(finite),
            max: obj.get("max").and_then(finite),
            step: obj.get("step").and_then(finite),
        },
        "text" => RowKind::Text {
            default: obj
                .get("default")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            max_len: obj
                .get("maxLen")
                .or_else(|| obj.get("max_len"))
                .and_then(Value::as_u64)
                .map(|n| n as usize),
        },
        _ => return None,
    };
    Some(Row {
        key,
        label,
        kind,
        visible_if: parse_visible_if(obj.get("visible_if")),
    })
}

fn parse_visible_if(raw: Option<&Value>) -> Option<VisibleIf> {
    let raw = match raw {
        None | Some(Value::Null) => return None,
        Some(value) => value,
    };
    let hidden = VisibleIf {
        key: String::new(),
        equals: None,
        not_equals: None,
    };
    let obj = match raw.as_object() {
        Some(obj) => obj,
        None => return Some(hidden),
    };
    let key = match obj.get("key").and_then(Value::as_str) {
        Some(key) => key.to_string(),
        None => return Some(hidden),
    };
    Some(VisibleIf {
        key,
        equals: obj.get("equals").filter(|v| !v.is_null()).cloned(),
        not_equals: obj.get("not_equals").filter(|v| !v.is_null()).cloned(),
    })
}

fn parse_choices(raw: Option<&Value>) -> Vec<(String, Value)> {
    let list = match raw.and_then(Value::as_array) {
        Some(list) => list,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for entry in list {
        match entry {
            Value::Array(pair) if pair.len() >= 2 => {
                if let Some(label) = scalar_label(&pair[0]) {
                    if is_scalar(&pair[1]) {
                        out.push((label, pair[1].clone()));
                    }
                }
            }
            other if is_scalar(other) => {
                if let Some(label) = scalar_label(other) {
                    out.push((label, other.clone()));
                }
            }
            _ => {}
        }
    }
    out
}

fn scalar_label(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn is_scalar(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_))
}

fn finite(value: &Value) -> Option<f64> {
    value.as_f64().filter(|n| n.is_finite())
}

fn number_value(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
        Value::from(n as i64)
    } else {
        serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

// ------- visibility

/// `visible_if` against the current values; an unset key compares as null.
pub fn visible(row: &Row, values: &HashMap<String, Value>) -> bool {
    let condition = match &row.visible_if {
        None => return true,
        Some(condition) => condition,
    };
    if condition.key.is_empty() {
        return false;
    }
    let current = values.get(&condition.key).cloned().unwrap_or(Value::Null);
    if let Some(equals) = &condition.equals {
        return &current == equals;
    }
    if let Some(not_equals) = &condition.not_equals {
        return &current != not_equals;
    }
    false
}

// ------- coercion

/// Exactly what a pin would store for this row, or why the value cannot be stored.
pub fn coerce(row: &Row, value: &Value) -> Result<Value, String> {
    match &row.kind {
        RowKind::Toggle { .. } => match value {
            Value::Bool(b) => Ok(Value::Bool(*b)),
            Value::String(s) if s == "true" || s == "false" => Ok(Value::Bool(s == "true")),
            _ => Err(format!("{} is a toggle; it takes true or false", row.key)),
        },
        RowKind::Number { min, max, step, .. } => {
            let raw = match value {
                Value::Number(_) => finite(value),
                Value::String(s) => s.trim().parse::<f64>().ok().filter(|n| n.is_finite()),
                _ => None,
            }
            .ok_or_else(|| format!("{} is a number row; it takes a finite number", row.key))?;
            Ok(number_value(snap(raw, *min, *max, *step)))
        }
        RowKind::Text { max_len, .. } => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{} is a text row; it takes a string", row.key))?;
            let limit = max_len
                .map(|n| n.min(MAX_OPTION_TEXT))
                .unwrap_or(MAX_OPTION_TEXT);
            Ok(Value::String(text.chars().take(limit).collect::<String>()))
        }
        RowKind::Choice { choices, .. } => {
            if choices
                .iter()
                .any(|(_, candidate)| same_scalar(candidate, value))
            {
                return Ok(value.clone());
            }
            Err(format!(
                "{} must be one of: {}",
                row.key,
                choices
                    .iter()
                    .map(|(_, v)| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
    }
}

/// JSON equality, except 1 and 1.0 are the same Lua number.
fn same_scalar(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        },
        _ => a == b,
    }
}

fn snap(value: f64, min: Option<f64>, max: Option<f64>, step: Option<f64>) -> f64 {
    let mut out = value;
    if let Some(step) = step.filter(|s| s.is_finite() && *s > 0.0) {
        let base = min.filter(|m| m.is_finite()).unwrap_or(0.0);
        out = base + ((out - base) / step).round() * step;
        out = (out * 1e10).round() / 1e10;
    }
    if let Some(min) = min.filter(|m| m.is_finite()) {
        if out < min {
            out = min;
        }
    }
    if let Some(max) = max.filter(|m| m.is_finite()) {
        if out > max {
            out = max;
        }
    }
    out
}

// ------- freezing

/// The `mods[].options` object for a pin: every value the user set, in row order,
/// then any key the rows do not describe. Drops carry the reason the UI shows.
pub fn freeze(rows: &[Row], values: &HashMap<String, Value>) -> Frozen {
    let mut frozen = Frozen::default();
    let mut taken: Vec<&String> = Vec::new();

    for row in rows {
        let value = match values.get(&row.key) {
            Some(value) => value,
            None => continue,
        };
        taken.push(&row.key);
        match coerce(row, value) {
            Ok(coerced) => push(&mut frozen, &row.key, coerced),
            Err(reason) => frozen.dropped.push(DroppedOption {
                key: row.key.clone(),
                reason,
            }),
        }
    }

    let mut extras: Vec<&String> = values
        .keys()
        .filter(|key| !taken.contains(key) && !frozen.options.contains_key(*key))
        .collect();
    extras.sort();
    for key in extras {
        match values.get(key) {
            Some(value) if is_scalar(value) => {
                let value = match value {
                    Value::String(text) => {
                        Value::String(text.chars().take(MAX_OPTION_TEXT).collect::<String>())
                    }
                    other => other.clone(),
                };
                push(&mut frozen, key, value);
            }
            _ => frozen.dropped.push(DroppedOption {
                key: key.clone(),
                reason: "options hold strings, numbers and booleans only".to_string(),
            }),
        }
    }

    frozen
}

fn push(frozen: &mut Frozen, key: &str, value: Value) {
    if key.is_empty() || key.chars().count() > MAX_OPTION_KEY {
        frozen.dropped.push(DroppedOption {
            key: key.to_string(),
            reason: format!("option keys are 1..{} characters", MAX_OPTION_KEY),
        });
        return;
    }
    if frozen.options.len() >= MAX_OPTIONS {
        frozen.dropped.push(DroppedOption {
            key: key.to_string(),
            reason: format!("a pin carries at most {} options", MAX_OPTIONS),
        });
        return;
    }
    frozen.options.insert(key.to_string(), value);
}

// ------- the sandboxed options_schema chunk

/// Run a mod's `options_schema` chunk with no io, os, package, debug, load or
/// filesystem in reach, under a step budget and a memory cap.
pub fn evaluate_lua_schema(source: &str) -> Result<Vec<Row>, String> {
    let value = run_sandboxed(source)?;
    let rows = value
        .as_array()
        .ok_or_else(|| "options_schema must return an array of rows".to_string())?;
    Ok(parse_rows(rows))
}

fn run_sandboxed(source: &str) -> Result<Value, String> {
    use mlua::{ChunkMode, HookTriggers, Lua, LuaOptions, StdLib};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    if source.as_bytes().first() == Some(&0x1b) {
        return Err("options_schema must be Lua source, not bytecode".to_string());
    }

    let lua = Lua::new_with(
        StdLib::MATH | StdLib::STRING | StdLib::TABLE,
        LuaOptions::default(),
    )
    .map_err(|e| format!("sandbox unavailable: {}", e))?;
    lua.set_memory_limit(MEMORY_LIMIT)
        .map_err(|e| format!("sandbox unavailable: {}", e))?;

    let env = restricted_env(&lua).map_err(|e| format!("sandbox unavailable: {}", e))?;

    let budget = STEP_LIMIT / u64::from(STEP_GRANULARITY);
    let ticks = Arc::new(AtomicU64::new(0));
    let counter = Arc::clone(&ticks);
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(STEP_GRANULARITY),
        move |_, _| {
            if counter.fetch_add(1, Ordering::Relaxed) >= budget {
                return Err(mlua::Error::RuntimeError(
                    "options_schema exceeded its step limit".to_string(),
                ));
            }
            Ok(())
        },
    );

    let result: mlua::Result<mlua::Value> = lua
        .load(source)
        .set_name("=options_schema")
        .set_mode(ChunkMode::Text)
        .set_environment(env)
        .eval();
    lua.remove_hook();

    let value = result.map_err(short_lua_error)?;
    let mut nodes = 0usize;
    lua_to_json(&value, 0, &mut nodes)
}

fn short_lua_error(err: mlua::Error) -> String {
    let text = err.to_string();
    let line = text.lines().next().unwrap_or("options_schema failed");
    line.chars().take(200).collect()
}

fn restricted_env(lua: &mlua::Lua) -> mlua::Result<mlua::Table<'_>> {
    let globals = lua.globals();
    // string.dump hands out bytecode, and the string metatable shares this table.
    if let Ok(strings) = globals.get::<_, mlua::Table>("string") {
        strings.raw_set("dump", mlua::Value::Nil)?;
    }

    let env = lua.create_table()?;
    for name in [
        "assert",
        "error",
        "ipairs",
        "next",
        "pairs",
        "pcall",
        "select",
        "tonumber",
        "tostring",
        "type",
        "xpcall",
        "rawequal",
        "setmetatable",
        "getmetatable",
        "unpack",
        "_VERSION",
    ] {
        let value: mlua::Value = globals.raw_get(name)?;
        if value != mlua::Value::Nil {
            env.raw_set(name, value)?;
        }
    }
    for name in ["math", "string", "table"] {
        let source: mlua::Table = globals.raw_get(name)?;
        let copy = lua.create_table()?;
        for pair in source.clone().pairs::<mlua::Value, mlua::Value>() {
            let (key, value) = pair?;
            copy.raw_set(key, value)?;
        }
        env.raw_set(name, copy)?;
    }
    env.raw_set("_G", env.clone())?;
    Ok(env)
}

fn lua_to_json(value: &mlua::Value, depth: usize, nodes: &mut usize) -> Result<Value, String> {
    *nodes += 1;
    if depth > MAX_LUA_DEPTH {
        return Err("options_schema nests too deeply".to_string());
    }
    if *nodes > MAX_LUA_NODES {
        return Err("options_schema returned too much data".to_string());
    }
    match value {
        mlua::Value::Nil => Ok(Value::Null),
        mlua::Value::Boolean(b) => Ok(Value::Bool(*b)),
        mlua::Value::Integer(n) => Ok(Value::from(*n)),
        mlua::Value::Number(n) => Ok(number_value(*n)),
        mlua::Value::String(s) => Ok(Value::String(s.to_string_lossy().into_owned())),
        mlua::Value::Table(table) => lua_table_to_json(table, depth, nodes),
        _ => Ok(Value::Null),
    }
}

pub(crate) fn lua_table_to_json(
    table: &mlua::Table,
    depth: usize,
    nodes: &mut usize,
) -> Result<Value, String> {
    let len = table.raw_len();
    let mut array = Vec::new();
    for index in 1..=len {
        let item: mlua::Value = table
            .raw_get(index)
            .map_err(|_| "options_schema table could not be read".to_string())?;
        array.push(lua_to_json(&item, depth + 1, nodes)?);
    }

    let mut object = Map::new();
    for pair in table.clone().pairs::<mlua::Value, mlua::Value>() {
        let (key, value) =
            pair.map_err(|_| "options_schema table could not be read".to_string())?;
        let name = match &key {
            mlua::Value::String(s) => s.to_string_lossy().into_owned(),
            mlua::Value::Integer(n) if *n >= 1 && (*n as usize) <= len => continue,
            mlua::Value::Integer(n) => n.to_string(),
            mlua::Value::Number(n) => n.to_string(),
            _ => continue,
        };
        object.insert(name, lua_to_json(&value, depth + 1, nodes)?);
    }

    if object.is_empty() {
        Ok(Value::Array(array))
    } else {
        for (index, item) in array.into_iter().enumerate() {
            object.insert((index + 1).to_string(), item);
        }
        Ok(Value::Object(object))
    }
}
