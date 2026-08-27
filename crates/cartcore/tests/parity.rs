//! Byte parity against fixtures produced by gen1recomp's tools/cartkit.py.
//! Regenerate with tests/fixtures/generate.py after an upstream format change.

use cartcore::cart::parse_cart;
use cartcore::findings::Severity;
use cartcore::labelart::label_art;
use cartcore::luaenc::{lua_string, lua_value};
use cartcore::pack::{bundle_bytes, packed_cart};
use cartcore::spec::{derive_id, parse_option, parse_spec, Spec};
use cartcore::validate::{range_problem, schema_findings};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read(name: &str) -> Vec<u8> {
    std::fs::read(fixtures().join(name)).unwrap_or_else(|_| panic!("fixture {}", name))
}

fn read_json(name: &str) -> Value {
    serde_json::from_slice(&read(name)).expect("fixture json")
}

/// Lay a fixture cart out on disk with its label art, as cartkit packed it.
fn stage(name: &str) -> (tempdir::TempDir, cartcore::Cart) {
    let dir = tempdir::TempDir::new("cartcore-parity").expect("temp dir");
    let body = String::from_utf8(read(&format!("{}.cart.json", name))).expect("utf-8");
    let cart = parse_cart(&body).expect("fixture cart parses");
    std::fs::write(dir.path().join("cart.json"), &body).expect("write cart");
    if let Some(label) = cart.get("label").and_then(Value::as_str) {
        let shell = cart
            .get("shell")
            .and_then(Value::as_str)
            .unwrap_or("#8b1a1a");
        let path = dir.path().join(label);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("label dir");
        }
        std::fs::write(path, label_art(shell)).expect("write label");
    }
    (dir, cart)
}

#[test]
fn packs_byte_identically_to_cartkit() {
    for name in ["good", "good_nolabel", "defaults", "torture", "minimal"] {
        let (dir, cart) = stage(name);
        let ours = bundle_bytes(&cart, Some(dir.path())).expect("pack");
        let theirs = read(&format!("{}.g1rcart", name));
        assert_eq!(
            String::from_utf8_lossy(&ours),
            String::from_utf8_lossy(&theirs),
            "{} bundle differs",
            name
        );
        assert_eq!(ours, theirs, "{} bundle bytes differ", name);
    }
}

#[test]
fn packing_is_deterministic_and_order_independent() {
    let (dir, cart) = stage("good");
    let first = bundle_bytes(&cart, Some(dir.path())).expect("pack");
    let second = bundle_bytes(&cart, Some(dir.path())).expect("pack");
    assert_eq!(first, second);

    let mut shuffled = serde_json::Map::new();
    for key in cart.keys().rev() {
        shuffled.insert(key.clone(), cart[key].clone());
    }
    let third = bundle_bytes(&shuffled, Some(dir.path())).expect("pack");
    assert_eq!(first, third, "key order changed the bundle");
}

#[test]
fn label_art_matches_cartkit() {
    use base64::Engine as _;
    let cases = read_json("label_art.json");
    for (shell, encoded) in cases.as_object().expect("object") {
        let want = base64::engine::general_purpose::STANDARD
            .decode(encoded.as_str().expect("base64"))
            .expect("base64 decodes");
        assert_eq!(label_art(shell), want, "label art for {}", shell);
    }
}

#[test]
fn lua_strings_match_cartkit() {
    for case in read_json("lua_strings.json").as_array().expect("array") {
        let text = case["text"].as_str().expect("text");
        let want = case["lua"].as_str().expect("lua");
        assert_eq!(lua_string(text), want, "escaping {:?}", text);
    }
}

#[test]
fn lua_numbers_match_cartkit() {
    for case in read_json("lua_numbers.json").as_array().expect("array") {
        let raw = case["json"].as_str().expect("json");
        let want = case["lua"].as_str().expect("lua");
        let value: Value = serde_json::from_str(raw).expect("number parses");
        assert_eq!(
            lua_value(&value, 0).expect("encodes"),
            want,
            "number {}",
            raw
        );
    }
}

#[test]
fn validation_matches_cartkit() {
    for case in read_json("validation.json").as_array().expect("array") {
        let name = case["name"].as_str().expect("name");
        let cart = case["cart"].as_object().expect("cart").clone();
        let ours = schema_findings(&cart, None);
        let theirs = case["findings"].as_array().expect("findings");
        let rendered: Vec<String> = ours
            .iter()
            .map(|f| format!("{} {} {}", f.rule, f.severity.as_str(), f.message))
            .collect();
        let expected: Vec<String> = theirs
            .iter()
            .map(|f| {
                format!(
                    "{} {} {}",
                    f["rule"].as_str().unwrap_or_default(),
                    f["severity"].as_str().unwrap_or_default(),
                    f["message"].as_str().unwrap_or_default()
                )
            })
            .collect();
        assert_eq!(rendered, expected, "findings for {}", name);
    }
}

#[test]
fn specs_match_cartkit() {
    let cases = read_json("specs.json");
    for case in cases["specs"].as_array().expect("array") {
        let text = case["spec"].as_str().expect("spec");
        let ours = parse_spec(text);
        match case["source"].as_str() {
            Some("github") => {
                let want_slug = case["target"].as_str().expect("slug");
                let want_version = case["version"].as_str().expect("version");
                assert_eq!(
                    ours,
                    Some(Spec::Github {
                        slug: want_slug.to_string(),
                        version: want_version.to_string()
                    }),
                    "spec {}",
                    text
                );
            }
            Some("gamebanana") => {
                let want = case["target"].as_u64().expect("mod id");
                assert_eq!(
                    ours,
                    Some(Spec::GameBanana { mod_id: want }),
                    "spec {}",
                    text
                );
            }
            _ => assert_eq!(ours, None, "spec {} should not parse", text),
        }
    }
    for case in cases["options"].as_array().expect("array") {
        let text = case["text"].as_str().expect("text");
        let nonfinite = case["nonfinite"].as_bool().unwrap_or(false);
        match parse_option(text) {
            Ok((key, value)) => {
                assert!(!nonfinite, "option {} should be refused", text);
                assert_eq!(key, case["key"].as_str().expect("key"), "option {}", text);
                assert_eq!(value, case["value"], "option {}", text);
            }
            Err(_) => assert!(
                case["key"].is_null() || nonfinite,
                "option {} should parse",
                text
            ),
        }
    }
    for case in cases["ids"].as_array().expect("array") {
        assert_eq!(
            derive_id(case["text"].as_str().expect("text")),
            case["id"].as_str().expect("id"),
            "derive_id {}",
            case["text"]
        );
    }
    for case in cases["ranges"].as_array().expect("array") {
        let text = case["range"].as_str().expect("range");
        let ours = range_problem(Some(&Value::String(text.to_string())));
        match case["problem"].as_str() {
            Some(want) => assert_eq!(ours.as_deref(), Some(want), "range {}", text),
            None => assert_eq!(ours, None, "range {} should be accepted", text),
        }
    }
}

#[test]
fn packed_cart_materializes_defaults() {
    let (_dir, cart) = stage("defaults");
    let packed = packed_cart(&cart);
    assert_eq!(packed["seal"], Value::String("sealed".into()));
    assert_eq!(
        packed["load_order"],
        serde_json::json!(["harder-trainers", "new-music"])
    );
    assert!(packed.get("schema").is_none());
}

#[test]
fn strict_is_what_pack_uses() {
    let (dir, cart) = stage("good");
    let report = cartcore::validate_cart(&cart, Some(dir.path()));
    assert!(report.ok(true), "the good fixture must pass strict");

    let mut placeholder = cart.clone();
    placeholder["mods"][0]["repo"] = Value::String("owner/example-mod".into());
    placeholder["mods"][0]["sha256"] = Value::String("0".repeat(64));
    let report = cartcore::validate_cart(&placeholder, Some(dir.path()));
    assert!(report.ok(false), "placeholders are warnings, not errors");
    assert!(!report.ok(true), "pack refuses a warning");
    assert_eq!(report.warnings().count(), 2);
    assert!(report.findings.iter().all(|f| f.severity == Severity::Warn));
}
