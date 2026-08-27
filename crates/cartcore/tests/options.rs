use cartcore::optionschema::{
    coerce, evaluate_lua_schema, freeze, parse_snapshot, visible, Row, RowKind,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mods")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(fixtures().join(rel)).expect("fixture")
}

fn escape(name: &str) -> Result<Vec<Row>, String> {
    evaluate_lua_schema(&read(&format!("escapes/{}", name)))
}

fn row(key: &str, kind: RowKind) -> Row {
    Row {
        key: key.to_string(),
        label: key.to_string(),
        kind,
        visible_if: None,
    }
}

fn values(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect()
}

// ------- snapshot

#[test]
fn snapshot_version_1_parses_every_row_type() {
    let mods = parse_snapshot(&read("snapshots/v1.json")).expect("v1 parses");
    let rows = mods.get("example").expect("example rows");
    assert_eq!(rows.len(), 4, "unknown type and keyless rows are skipped");
    assert_eq!(
        rows.iter().map(Row::type_name).collect::<Vec<_>>(),
        vec!["toggle", "choice", "number", "text"]
    );
    assert_eq!(mods.get("other").map(Vec::len), Some(0));
    assert!(
        !mods.contains_key("bad"),
        "a non-array mod entry is skipped"
    );

    match &rows[1].kind {
        RowKind::Choice { default, choices } => {
            assert_eq!(default, &json!("safe"));
            assert_eq!(choices[1], ("Fast".to_string(), json!("fast")));
        }
        other => panic!("expected a choice row, got {:?}", other),
    }
    match &rows[3].kind {
        RowKind::Text { max_len, .. } => assert_eq!(*max_len, Some(12)),
        other => panic!("expected a text row, got {:?}", other),
    }
}

#[test]
fn snapshot_missing_version_reads_as_1() {
    let mods = parse_snapshot(&read("snapshots/legacy_unversioned.json")).expect("legacy parses");
    assert_eq!(mods["example"].len(), 1);
}

#[test]
fn snapshot_newer_version_is_refused() {
    let err = parse_snapshot(&read("snapshots/v2.json")).expect_err("version 2 is refused");
    assert!(err.contains("schema_version 2"), "{}", err);
}

#[test]
fn snapshot_malformed_json_is_an_error_not_a_panic() {
    assert!(parse_snapshot(&read("snapshots/malformed.json")).is_err());
    assert!(parse_snapshot("").is_err());
    assert!(parse_snapshot("[]").is_err());
    assert!(parse_snapshot(r#"{"schema_version":"one","mods":{}}"#).is_err());
    assert!(parse_snapshot(r#"{"schema_version":1,"mods":[]}"#).is_err());
    assert!(parse_snapshot(r#"{"schema_version":1}"#)
        .expect("no mods")
        .is_empty());
}

// ------- visibility

#[test]
fn visible_if_equals_and_not_equals() {
    let mods = parse_snapshot(&read("snapshots/v1.json")).unwrap();
    let rows = &mods["example"];
    let rate = &rows[2];
    let name = &rows[3];

    assert!(visible(&rows[0], &values(&[])));
    assert!(visible(rate, &values(&[("mode", json!("fast"))])));
    assert!(!visible(rate, &values(&[("mode", json!("safe"))])));
    assert!(
        !visible(rate, &values(&[])),
        "an unset key compares as null"
    );

    assert!(visible(name, &values(&[("enabled", json!(true))])));
    assert!(!visible(name, &values(&[("enabled", json!(false))])));
}

#[test]
fn a_malformed_condition_hides_the_row() {
    let rows = parse_snapshot(
        r#"{"mods":{"m":[{"key":"a","type":"toggle","visible_if":"nonsense"},
                        {"key":"b","type":"toggle","visible_if":{"key":"a"}}]}}"#,
    )
    .unwrap();
    let rows = &rows["m"];
    assert!(!visible(&rows[0], &values(&[])));
    assert!(!visible(&rows[1], &values(&[("a", json!(true))])));
}

// ------- coercion

#[test]
fn toggle_coercion() {
    let toggle = row("enabled", RowKind::Toggle { default: true });
    assert_eq!(coerce(&toggle, &json!(false)).unwrap(), json!(false));
    assert_eq!(coerce(&toggle, &json!("true")).unwrap(), json!(true));
    assert!(coerce(&toggle, &json!(1)).is_err());
}

#[test]
fn number_coercion_clamps_and_snaps() {
    let number = row(
        "rate",
        RowKind::Number {
            default: 5.0,
            min: Some(0.0),
            max: Some(10.0),
            step: Some(2.0),
        },
    );
    assert_eq!(coerce(&number, &json!(99)).unwrap(), json!(10));
    assert_eq!(coerce(&number, &json!(-4)).unwrap(), json!(0));
    assert_eq!(coerce(&number, &json!(5)).unwrap(), json!(6));
    assert_eq!(coerce(&number, &json!("3")).unwrap(), json!(4));
    assert!(coerce(&number, &json!("nope")).is_err());
    assert!(coerce(&number, &json!(true)).is_err());

    let free = row(
        "ratio",
        RowKind::Number {
            default: 0.0,
            min: None,
            max: None,
            step: Some(0.1),
        },
    );
    assert_eq!(coerce(&free, &json!(0.34)).unwrap(), json!(0.3));
}

#[test]
fn text_coercion_honours_max_len_and_the_manifest_cap() {
    let short = row(
        "name",
        RowKind::Text {
            default: String::new(),
            max_len: Some(4),
        },
    );
    assert_eq!(coerce(&short, &json!("abcdefg")).unwrap(), json!("abcd"));
    assert!(coerce(&short, &json!(7)).is_err());

    let wide = row(
        "note",
        RowKind::Text {
            default: String::new(),
            max_len: Some(9999),
        },
    );
    let long = "x".repeat(400);
    let frozen = coerce(&wide, &json!(long)).unwrap();
    assert_eq!(frozen.as_str().unwrap().chars().count(), 256);
}

#[test]
fn choice_coercion_rejects_values_outside_the_choices() {
    let choice = row(
        "mode",
        RowKind::Choice {
            default: json!("safe"),
            choices: vec![
                ("Safe".to_string(), json!("safe")),
                ("Fast".to_string(), json!("fast")),
            ],
        },
    );
    assert_eq!(coerce(&choice, &json!("fast")).unwrap(), json!("fast"));
    let err = coerce(&choice, &json!("turbo")).unwrap_err();
    assert!(err.contains("\"safe\""), "{}", err);
}

// ------- freezing

#[test]
fn freeze_writes_every_value_including_defaults() {
    let mods = parse_snapshot(&read("snapshots/v1.json")).unwrap();
    let rows = &mods["example"];
    let frozen = freeze(
        rows,
        &values(&[
            ("enabled", json!(true)),
            ("mode", json!("fast")),
            ("rate", json!(42)),
            ("name", json!("ASH")),
        ]),
    );
    assert!(frozen.dropped.is_empty());
    assert_eq!(
        frozen.options,
        json!({"enabled": true, "mode": "fast", "rate": 10, "name": "ASH"})
            .as_object()
            .unwrap()
            .clone()
    );
    assert_eq!(
        frozen.options.keys().collect::<Vec<_>>(),
        vec!["enabled", "mode", "rate", "name"],
        "row order is preserved"
    );
}

#[test]
fn freeze_reports_dropped_values() {
    let mods = parse_snapshot(&read("snapshots/v1.json")).unwrap();
    let rows = &mods["example"];
    let frozen = freeze(
        rows,
        &values(&[
            ("mode", json!("turbo")),
            ("loose", json!([1, 2])),
            ("free", json!("kept")),
        ]),
    );
    assert_eq!(frozen.options.get("free"), Some(&json!("kept")));
    let dropped: Vec<&str> = frozen.dropped.iter().map(|d| d.key.as_str()).collect();
    assert_eq!(dropped, vec!["mode", "loose"]);
}

#[test]
fn freeze_enforces_the_option_and_key_caps() {
    let rows: Vec<Row> = (0..70)
        .map(|i| row(&format!("k{:02}", i), RowKind::Toggle { default: false }))
        .collect();
    let all: HashMap<String, Value> = rows
        .iter()
        .map(|row| (row.key.clone(), json!(true)))
        .collect();
    let frozen = freeze(&rows, &all);
    assert_eq!(frozen.options.len(), 64);
    assert_eq!(frozen.dropped.len(), 6);
    assert!(frozen.dropped[0].reason.contains("at most 64 options"));

    let long_key = "k".repeat(65);
    let rows = vec![row(&long_key, RowKind::Toggle { default: false })];
    let frozen = freeze(&rows, &values(&[(long_key.as_str(), json!(true))]));
    assert!(frozen.options.is_empty());
    assert!(frozen.dropped[0].reason.contains("1..64 characters"));
}

// ------- the sandboxed options_schema chunk

#[test]
fn a_real_options_schema_chunk_evaluates() {
    let rows = evaluate_lua_schema(&read("example_mod/options_schema.lua")).expect("schema runs");
    assert_eq!(
        rows.iter().map(Row::type_name).collect::<Vec<_>>(),
        vec!["toggle", "choice", "number", "text", "toggle", "toggle", "toggle"]
    );
    assert_eq!(rows[0].label, "Enabled");
    assert_eq!(rows[4].key, "extra1", "the loop-built rows survive");
    match &rows[2].kind {
        RowKind::Number { min, max, step, .. } => {
            assert_eq!((*min, *max, *step), (Some(0.0), Some(10.0), Some(2.0)));
        }
        other => panic!("expected a number row, got {:?}", other),
    }
    assert!(!visible(&rows[2], &values(&[("mode", json!("safe"))])));
}

#[test]
fn a_row_key_defaults_to_the_label() {
    let rows = evaluate_lua_schema(r#"return { { key = "solo", type = "toggle" } }"#).unwrap();
    assert_eq!(rows[0].label, "solo");
}

#[test]
fn a_chunk_that_returns_a_non_table_is_an_error() {
    assert!(escape("not_a_table.lua").is_err());
    assert!(escape("syntax_error.lua").is_err());
    assert!(evaluate_lua_schema("\u{1b}Lua bytecode").is_err());
}

#[test]
fn the_sandbox_has_no_io_os_package_or_debug() {
    for name in [
        "io.lua",
        "os.lua",
        "require.lua",
        "love.lua",
        "dofile.lua",
        "loadfile.lua",
        "loadstring.lua",
        "package_loadlib.lua",
        "debug.lua",
        "string_dump.lua",
        "collectgarbage.lua",
    ] {
        let err = escape(name).expect_err(name);
        assert!(
            err.contains("nil value") || err.contains("attempt to index"),
            "{}: {}",
            name,
            err
        );
    }
}

#[test]
fn the_string_metatable_does_not_lead_back_to_dump() {
    let rows = escape("metatable_escape.lua").expect("runs");
    assert_eq!(rows[0].default_value(), json!(false));
}

#[test]
fn an_infinite_loop_hits_the_step_limit() {
    let err = escape("infinite_loop.lua").expect_err("the loop is stopped");
    assert!(err.contains("step limit"), "{}", err);
}

#[test]
fn a_runaway_allocation_hits_the_memory_cap() {
    let err = escape("runaway_memory.lua").expect_err("the allocation is refused");
    assert!(err.contains("memory"), "{}", err);
}
