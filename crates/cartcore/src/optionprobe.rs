//! Recovers the option rows a mod registers at runtime with `mod.options:define`.
//!
//! Four of the 145 indexed mods publish a manifest `options_schema`; eighty-six
//! call `define` from their entry chunk instead, so reading the manifest alone
//! finds almost nothing. This runs the entry in the same sandbox as a schema
//! chunk, behind a stub engine, and records what it registers.
//!
//! The rows are the mod's own, but the run is a guess: a mod that varies its
//! schema by generation or by what the engine reports gets whatever the stubs
//! led it to. Callers must label the result as derived, never authoritative.

use crate::optionschema::{parse_rows, Row};
use mlua::{ChunkMode, HookTriggers, Lua, LuaOptions, StdLib};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Entry chunks do real work before and after `define`, so this is wider than
/// the schema-chunk budget.
const STEP_LIMIT: u64 = 12_000_000;
const STEP_GRANULARITY: u32 = 10_000;
const MEMORY_LIMIT: usize = 64 * 1024 * 1024;
const MAX_REQUIRE_DEPTH: usize = 32;
/// A click must not hang the window: the step hook is blind to time spent
/// inside a C function, so the run also carries a wall clock.
const TIME_LIMIT: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    pub rows: Vec<Row>,
    /// Set when the chunk failed after registering rows; the rows still stand.
    pub note: Option<String>,
}

/// The mod's Lua sources, keyed by their path inside the archive.
pub type Sources = HashMap<String, String>;

/// Strip a single wrapping directory so `require` names resolve either way.
pub fn normalize_sources(raw: &Sources) -> Sources {
    let mut roots = raw
        .keys()
        .filter_map(|name| name.split('/').next())
        .collect::<Vec<_>>();
    roots.sort_unstable();
    roots.dedup();
    let nested = raw.keys().all(|name| name.contains('/'));
    if roots.len() == 1 && nested {
        let prefix = format!("{}/", roots[0]);
        return raw
            .iter()
            .map(|(name, body)| (name.trim_start_matches(&prefix).to_string(), body.clone()))
            .collect();
    }
    raw.clone()
}

/// Candidate archive paths for one `require` name.
fn require_candidates(name: &str) -> Vec<String> {
    let slashed = name.replace('.', "/");
    let mut out = vec![
        format!("{}.lua", slashed),
        slashed.clone(),
        format!("{}/init.lua", slashed),
    ];
    if let Some(tail) = slashed.rsplit('/').next() {
        out.push(format!("{}.lua", tail));
    }
    out
}

fn short_error(err: &mlua::Error) -> String {
    let text = err.to_string();
    let line = text.lines().next().unwrap_or("the mod's entry failed");
    line.chars().take(200).collect()
}

/// Run `entry` with a stub engine and collect every schema it defines.
///
/// Two passes: one telling the mod its stubbed engine members are functions,
/// one calling them tables. Mods guard both ways, and the pass that registers
/// rows wins. A clean run beats a run that registered the same rows and then
/// died.
pub fn probe_entry(
    sources: &Sources,
    entry: &str,
    id: &str,
    version: &str,
) -> Result<Probe, String> {
    let sources = normalize_sources(sources);
    let entry_source = sources
        .get(entry)
        .or_else(|| sources.get(&format!("{}.lua", entry.trim_end_matches(".lua"))))
        .ok_or_else(|| format!("the archive has no {}", entry))?
        .clone();
    if entry_source.as_bytes().first() == Some(&0x1b) {
        return Err("the entry must be Lua source, not bytecode".to_string());
    }

    let mut best: Option<Probe> = None;
    let mut first_problem: Option<String> = None;
    for word in ["function", "table"] {
        match probe_pass(&sources, &entry_source, id, version, word) {
            Ok(probe) if probe.note.is_none() => return Ok(probe),
            Ok(probe) => {
                if best.is_none() {
                    best = Some(probe);
                }
            }
            Err(problem) => {
                if first_problem.is_none() {
                    first_problem = Some(problem);
                }
            }
        }
    }
    best.ok_or_else(|| {
        first_problem.unwrap_or_else(|| "the mod's entry registered no options".to_string())
    })
}

fn probe_pass(
    sources: &Sources,
    entry_source: &str,
    id: &str,
    version: &str,
    stub_type: &'static str,
) -> Result<Probe, String> {
    let lua = Lua::new_with(
        StdLib::MATH | StdLib::STRING | StdLib::TABLE,
        LuaOptions::default(),
    )
    .map_err(|e| format!("sandbox unavailable: {}", e))?;
    lua.set_memory_limit(MEMORY_LIMIT)
        .map_err(|e| format!("sandbox unavailable: {}", e))?;

    let captured: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let env = build_env(&lua, sources, &captured, id, version, stub_type)
        .map_err(|e| format!("sandbox: {}", e))?;

    let budget = STEP_LIMIT / u64::from(STEP_GRANULARITY);
    let ticks = Arc::new(AtomicU64::new(0));
    let counter = Arc::clone(&ticks);
    let deadline = std::time::Instant::now() + TIME_LIMIT;
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(STEP_GRANULARITY),
        move |_, _| {
            if counter.fetch_add(1, Ordering::Relaxed) >= budget {
                return Err(mlua::Error::RuntimeError(
                    "the mod's entry exceeded its step limit".to_string(),
                ));
            }
            if std::time::Instant::now() >= deadline {
                return Err(mlua::Error::RuntimeError(
                    "the mod's entry ran too long".to_string(),
                ));
            }
            Ok(())
        },
    );

    let outcome = run_entry(&lua, &env, entry_source);
    lua.remove_hook();

    let schemas = captured.lock().map(|got| got.clone()).unwrap_or_default();
    let mut rows: Vec<Row> = Vec::new();
    for schema in &schemas {
        if let Some(array) = schema.as_array() {
            for row in parse_rows(array) {
                if !rows.iter().any(|seen| seen.key == row.key) {
                    rows.push(row);
                }
            }
        }
    }

    match outcome {
        Ok(()) if rows.is_empty() => {
            Err("the mod's entry ran but registered no options".to_string())
        }
        Ok(()) => Ok(Probe { rows, note: None }),
        Err(problem) if rows.is_empty() => Err(short_error(&problem)),
        Err(problem) => Ok(Probe {
            rows,
            note: Some(short_error(&problem)),
        }),
    }
}

/// Run the entry the way the engine does.
///
/// `Loader.lua:1605` calls the chunk with the mod handle, so `local mod = ...`
/// at the top of a file is the common shape; the other shape returns a
/// `function(mod)` to be called in turn.
fn run_entry(lua: &Lua, env: &mlua::Table<'_>, source: &str) -> mlua::Result<()> {
    let handle: mlua::Value = env.raw_get("mod")?;
    let value: mlua::Value = lua
        .load(source)
        .set_name("=entry")
        .set_mode(ChunkMode::Text)
        .set_environment(env.clone())
        .call(handle.clone())?;
    if let mlua::Value::Function(entry) = value {
        entry.call::<_, mlua::MultiValue>((handle,))?;
    }
    Ok(())
}

/// A callable, indexable stand-in for anything the engine would have provided.
///
/// `debug.setmetatable` is unavailable in a safe state, so the stub is a table
/// with `__call` and a `type` override rather than a real function.
fn make_stub(lua: &Lua) -> mlua::Result<mlua::Table<'_>> {
    let stub = lua.create_table()?;
    lua.set_named_registry_value("g1r_stub", stub.clone())?;
    let meta = lua.create_table()?;
    let hand_back = lua.create_function(|inner, _: mlua::MultiValue| {
        inner.named_registry_value::<mlua::Table>("g1r_stub")
    })?;
    meta.raw_set("__index", hand_back.clone())?;
    meta.raw_set("__call", hand_back)?;
    meta.raw_set(
        "__newindex",
        lua.create_function(|_, _: mlua::MultiValue| Ok(()))?,
    )?;
    meta.raw_set(
        "__tostring",
        lua.create_function(|_, _: mlua::MultiValue| Ok("stub"))?,
    )?;
    // Mods build paths like `api.paths.root .. "/x"`; a stub has to vanish
    // from a concatenation rather than raise.
    meta.raw_set(
        "__concat",
        lua.create_function(|_, args: mlua::MultiValue| {
            let text = args
                .into_iter()
                .filter_map(|value| match value {
                    mlua::Value::String(text) => Some(text.to_string_lossy().to_string()),
                    mlua::Value::Integer(n) => Some(n.to_string()),
                    mlua::Value::Number(n) => Some(n.to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("");
            Ok(text)
        })?,
    )?;
    stub.set_metatable(Some(meta));
    Ok(stub)
}

/// `type()` that calls the stub whatever this pass is pretending it is.
///
/// Mods guard their engine API both ways, and one word cannot satisfy both, so
/// a probe runs twice and keeps the pass that registered rows.
fn install_type(lua: &Lua, env: &mlua::Table<'_>, word: &'static str) -> mlua::Result<()> {
    let real: mlua::Function = lua.globals().raw_get("type")?;
    lua.set_named_registry_value("g1r_type", real)?;
    let typed = lua.create_function(move |inner, value: mlua::Value| {
        if let mlua::Value::Table(table) = &value {
            let stub: mlua::Table = inner.named_registry_value("g1r_stub")?;
            if table.equals(&stub)? {
                return Ok(word.to_string());
            }
        }
        let real: mlua::Function = inner.named_registry_value("g1r_type")?;
        real.call::<_, String>((value,))
    })?;
    env.raw_set("type", typed)?;
    Ok(())
}

fn build_env<'lua>(
    lua: &'lua Lua,
    sources: &Sources,
    captured: &Arc<Mutex<Vec<Value>>>,
    id: &str,
    version: &str,
    stub_type: &'static str,
) -> mlua::Result<mlua::Table<'lua>> {
    let globals = lua.globals();
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
        "rawget",
        "rawset",
        "rawlen",
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
    env.raw_set(
        "print",
        lua.create_function(|_, _: mlua::MultiValue| Ok(()))?,
    )?;

    let stub = make_stub(lua)?;
    install_type(lua, &env, stub_type)?;
    // Mods reach for os.time and os.clock to seed timers. Nothing that touches
    // the machine is offered: no execute, remove, rename, getenv or exit.
    let os = lua.create_table()?;
    os.raw_set(
        "time",
        lua.create_function(|_, _: mlua::MultiValue| Ok(0i64))?,
    )?;
    os.raw_set(
        "clock",
        lua.create_function(|_, _: mlua::MultiValue| Ok(0.0f64))?,
    )?;
    os.raw_set(
        "date",
        lua.create_function(|_, _: mlua::MultiValue| Ok("1970-01-01"))?,
    )?;
    os.raw_set(
        "getenv",
        lua.create_function(|_, _: mlua::MultiValue| Ok(mlua::Value::Nil))?,
    )?;
    env.raw_set("os", os)?;
    // Several mods ship their modules as embedded strings and `load` them, so an
    // inert loader means their `define` never runs. The compiled chunk inherits
    // this same environment, so it is no freer than the entry that built it.
    let loader = lua.create_function(|inner, args: mlua::MultiValue| {
        let mut args = args.into_iter();
        let source = match args.next() {
            Some(mlua::Value::String(text)) => text.as_bytes().to_vec(),
            _ => {
                return Ok(mlua::Value::Table(
                    inner.named_registry_value::<mlua::Table>("g1r_stub")?,
                ))
            }
        };
        if source.first() == Some(&0x1b) {
            return Ok(mlua::Value::Table(
                inner.named_registry_value::<mlua::Table>("g1r_stub")?,
            ));
        }
        let env: mlua::Table = inner.named_registry_value("g1r_env")?;
        match inner
            .load(&source)
            .set_name("=loaded")
            .set_mode(ChunkMode::Text)
            .set_environment(env)
            .into_function()
        {
            Ok(chunk) => Ok(mlua::Value::Function(chunk)),
            Err(_) => Ok(mlua::Value::Table(
                inner.named_registry_value::<mlua::Table>("g1r_stub")?,
            )),
        }
    })?;
    env.raw_set("load", loader.clone())?;
    env.raw_set("loadstring", loader)?;
    env.raw_set("love", stub.clone())?;

    // define(schema) and options:define(schema) both land here.
    let sink = Arc::clone(captured);
    let define = lua.create_function(move |_, args: mlua::MultiValue| {
        let schema = args
            .into_iter()
            .rev()
            .find(|value| matches!(value, mlua::Value::Table(_)));
        if let Some(mlua::Value::Table(table)) = schema {
            let mut nodes = 0usize;
            if let Ok(json) = crate::optionschema::lua_table_to_json(&table, 0, &mut nodes) {
                if let Ok(mut got) = sink.lock() {
                    got.push(json);
                }
            }
        }
        Ok(())
    })?;
    let options = lua.create_table()?;
    options.raw_set("define", define)?;
    let options_meta = lua.create_table()?;
    options_meta.raw_set("__index", stub.clone())?;
    options.set_metatable(Some(options_meta));

    let handle = lua.create_table()?;
    handle.raw_set("options", options)?;
    handle.raw_set("id", id)?;
    handle.raw_set("path", format!("mods/{}", id))?;
    handle.raw_set("version", version)?;
    handle.raw_set("api", 2)?;
    // `mod:read(rel)` is how a bundled mod pulls its own sources back out before
    // `load`ing them; without it those mods never build a module to define from.
    let files: HashMap<String, String> = sources.clone();
    let read = lua.create_function(move |inner, args: mlua::MultiValue| {
        let wanted = args
            .into_iter()
            .filter_map(|value| match value {
                mlua::Value::String(text) => Some(text.to_string_lossy().to_string()),
                _ => None,
            })
            .next_back();
        let Some(name) = wanted else {
            return Ok(mlua::Value::Nil);
        };
        let trimmed = name.trim_start_matches("./").to_string();
        let hit = files
            .get(&trimmed)
            .or_else(|| files.get(&format!("{}.lua", trimmed)))
            .or_else(|| {
                trimmed.rsplit('/').next().and_then(|tail| {
                    files
                        .get(tail)
                        .or_else(|| files.get(&format!("{}.lua", tail)))
                })
            });
        match hit {
            Some(body) => Ok(mlua::Value::String(inner.create_string(body)?)),
            None => Ok(mlua::Value::Nil),
        }
    })?;
    handle.raw_set("read", read)?;
    let handle_meta = lua.create_table()?;
    handle_meta.raw_set("__index", stub.clone())?;
    handle.set_metatable(Some(handle_meta));
    env.raw_set("mod", handle)?;

    install_require(lua, &env, sources)?;
    env.raw_set("_G", env.clone())?;
    Ok(env)
}

/// `require` serves the mod's own files and stubs everything else.
///
/// The env, the stub and the loaded-module table live in the registry because a
/// Lua closure has to be `'static` and cannot hold borrowed Lua values.
fn install_require(lua: &Lua, env: &mlua::Table<'_>, sources: &Sources) -> mlua::Result<()> {
    let files: HashMap<String, String> = sources.clone();
    lua.set_named_registry_value("g1r_env", env.clone())?;
    lua.set_named_registry_value("g1r_loaded", lua.create_table()?)?;
    let depth = Arc::new(AtomicU64::new(0));

    let require = lua.create_function(move |inner, name: String| {
        let stub: mlua::Value =
            mlua::Value::Table(inner.named_registry_value::<mlua::Table>("g1r_stub")?);
        let loaded: mlua::Table = inner.named_registry_value("g1r_loaded")?;
        let hit: mlua::Value = loaded.raw_get(name.clone())?;
        if hit != mlua::Value::Nil {
            return Ok(hit);
        }
        let found = require_candidates(&name)
            .into_iter()
            .find_map(|path| files.get(&path).cloned());
        let body = match found {
            Some(body) => body,
            None => return Ok(stub),
        };
        if depth.fetch_add(1, Ordering::Relaxed) as usize >= MAX_REQUIRE_DEPTH {
            depth.fetch_sub(1, Ordering::Relaxed);
            return Ok(stub);
        }
        let env: mlua::Table = inner.named_registry_value("g1r_env")?;
        let result: mlua::Result<mlua::Value> = inner
            .load(&body)
            .set_name(format!("={}", name))
            .set_mode(ChunkMode::Text)
            .set_environment(env)
            .call(inner.named_registry_value::<mlua::Table>("g1r_stub")?);
        depth.fetch_sub(1, Ordering::Relaxed);
        let value = match result {
            Ok(mlua::Value::Nil) => stub,
            Ok(value) => value,
            Err(_) => stub,
        };
        loaded.raw_set(name, value.clone())?;
        Ok(value)
    })?;
    env.raw_set("require", require)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources(pairs: &[(&str, &str)]) -> Sources {
        pairs
            .iter()
            .map(|(name, body)| (name.to_string(), body.to_string()))
            .collect()
    }

    #[test]
    fn captures_a_define_from_the_usual_entry_shape() {
        let src = sources(&[(
            "main.lua",
            r#"return function(mod)
                 mod.options:define({
                   { key = "ride", label = "RIDE", type = "choice", default = "on",
                     choices = { { "ON", "on" }, { "OFF", "off" } } },
                 })
               end"#,
        )]);
        let probe = probe_entry(&src, "main.lua", "demo", "1.0.0").unwrap();
        assert_eq!(probe.rows.len(), 1);
        assert_eq!(probe.rows[0].key, "ride");
        assert_eq!(probe.note, None);
    }

    #[test]
    fn captures_a_define_called_at_chunk_level() {
        let src = sources(&[(
            "main.lua",
            r#"mod.options:define({ { key = "on", label = "ON", type = "toggle", default = true } })"#,
        )]);
        let probe = probe_entry(&src, "main.lua", "demo", "1.0.0").unwrap();
        assert_eq!(probe.rows.len(), 1);
    }

    #[test]
    fn a_missing_engine_module_is_a_stub_the_mod_can_call() {
        let src = sources(&[(
            "main.lua",
            r#"local Font = require("src.render.Font")
               local w = Font.width("hi")
               assert(type(Font.width) == "function")
               return function(mod)
                 mod.options:define({ { key = "k", label = "L", type = "toggle", default = false } })
               end"#,
        )]);
        assert_eq!(
            probe_entry(&src, "main.lua", "demo", "1.0.0")
                .unwrap()
                .rows
                .len(),
            1
        );
    }

    #[test]
    fn the_mods_own_files_are_required_for_real() {
        let src = sources(&[
            (
                "main.lua",
                r#"local S = require("settings")
                   return function(mod) mod.options:define(S.schema()) end"#,
            ),
            (
                "settings.lua",
                r#"local M = {}
                   function M.schema()
                     return { { key = "rate", label = "RATE", type = "number",
                                default = 5, min = 0, max = 10, step = 1 } }
                   end
                   return M"#,
            ),
        ]);
        let probe = probe_entry(&src, "main.lua", "demo", "1.0.0").unwrap();
        assert_eq!(probe.rows.len(), 1);
        assert_eq!(probe.rows[0].key, "rate");
    }

    #[test]
    fn rows_survive_a_crash_that_happens_after_define() {
        let src = sources(&[(
            "main.lua",
            r#"return function(mod)
                 mod.options:define({ { key = "k", label = "L", type = "toggle", default = true } })
                 error("boom")
               end"#,
        )]);
        let probe = probe_entry(&src, "main.lua", "demo", "1.0.0").unwrap();
        assert_eq!(probe.rows.len(), 1);
        assert!(probe.note.is_some(), "the crash must be reported");
    }

    #[test]
    fn a_mod_with_no_options_is_an_error_not_an_empty_list() {
        let src = sources(&[("main.lua", "return function(mod) end")]);
        assert!(probe_entry(&src, "main.lua", "demo", "1.0.0").is_err());
    }

    #[test]
    fn duplicate_keys_are_kept_once() {
        let src = sources(&[(
            "main.lua",
            r#"return function(mod)
                 mod.options:define({ { key = "k", label = "A", type = "toggle", default = true } })
                 mod.options:define({ { key = "k", label = "B", type = "toggle", default = false } })
               end"#,
        )]);
        assert_eq!(
            probe_entry(&src, "main.lua", "demo", "1.0.0")
                .unwrap()
                .rows
                .len(),
            1
        );
    }

    #[test]
    fn an_endless_loop_is_stopped_by_the_step_limit() {
        let src = sources(&[("main.lua", "while true do end")]);
        assert!(probe_entry(&src, "main.lua", "demo", "1.0.0").is_err());
    }

    #[test]
    fn the_sandbox_offers_no_io_and_only_clocks_from_os() {
        let src = sources(&[(
            "main.lua",
            r#"return function(mod)
                 assert(io == nil, "io must be absent")
                 assert(os.execute == nil and os.remove == nil and os.exit == nil,
                        "os must offer nothing that touches the machine")
                 assert(type(os.time) == "function", "os.time is offered")
                 assert(load ~= nil, "load compiles into this same sandbox")
                 mod.options:define({ { key = "k", label = "L", type = "toggle", default = true } })
               end"#,
        )]);
        assert_eq!(
            probe_entry(&src, "main.lua", "demo", "1.0.0")
                .unwrap()
                .rows
                .len(),
            1
        );
    }

    #[test]
    fn bytecode_is_refused() {
        let src = sources(&[("main.lua", "\u{1b}Lua fake bytecode")]);
        assert!(probe_entry(&src, "main.lua", "demo", "1.0.0").is_err());
    }

    #[test]
    fn a_single_wrapping_directory_is_stripped() {
        let src = sources(&[(
            "a_autofire/main.lua",
            r#"return function(mod)
                 mod.options:define({ { key = "k", label = "L", type = "toggle", default = true } })
               end"#,
        )]);
        assert_eq!(
            probe_entry(&src, "main.lua", "demo", "1.0.0")
                .unwrap()
                .rows
                .len(),
            1
        );
    }
}

/// Measures the probe against every real mod in a directory of extracted
/// archives. Ignored by default; used to check coverage, not in CI.
#[cfg(test)]
mod live {
    use super::*;

    #[test]
    #[ignore]
    fn coverage_over_real_mods() {
        let root = std::path::PathBuf::from(
            std::env::var("G1R_MODS").unwrap_or_else(|_| "/tmp/g1r-mods".to_string()),
        );
        let mut total = 0usize;
        let mut got = 0usize;
        let mut misses: Vec<String> = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(&root)
            .expect("extract mods first")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        entries.sort();
        for dir in entries {
            let name = dir.file_name().unwrap().to_string_lossy().to_string();
            let manifest_path = dir.join("manifest.json");
            let Ok(text) = std::fs::read_to_string(&manifest_path) else {
                continue;
            };
            let manifest: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let entry = manifest
                .get("entry")
                .and_then(Value::as_str)
                .unwrap_or("main.lua")
                .to_string();
            let mut sources = Sources::new();
            collect(&dir, &dir, &mut sources);
            if !sources.values().any(|body| body.contains("options:define")) {
                continue;
            }
            total += 1;
            match probe_entry(
                &sources,
                &entry,
                &name,
                manifest
                    .get("version")
                    .and_then(Value::as_str)
                    .unwrap_or("0.0.0"),
            ) {
                Ok(probe) => {
                    got += 1;
                    println!(
                        "{:<34} {:>2} rows{}",
                        name,
                        probe.rows.len(),
                        probe
                            .note
                            .map(|n| format!("  (after: {})", n))
                            .unwrap_or_default()
                    );
                }
                Err(problem) => misses.push(format!("{:<34} {}", name, problem)),
            }
        }
        println!(
            "\n--- {} of {} mods with define() recovered ---",
            got, total
        );
        for miss in &misses {
            println!("MISS {}", miss);
        }
    }

    fn collect(root: &std::path::Path, dir: &std::path::Path, out: &mut Sources) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                collect(root, &path, out);
            } else {
                if let Ok(body) = std::fs::read_to_string(&path) {
                    let key = path
                        .strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.insert(key, body);
                }
            }
        }
    }
}
