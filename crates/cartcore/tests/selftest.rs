//! cartkit's own selftest case set, check for check.

use cartcore::cart::Cart;
use cartcore::findings::{Finding, Severity};
use cartcore::labelart::label_art;
use cartcore::luaenc::{lua_encode, lua_string, lua_value};
use cartcore::pack::{bundle_table, packed_cart};
use cartcore::spec::{parse_option, parse_spec, Spec};
use cartcore::validate::{range_problem, schema_findings};
use serde_json::{json, Value};

fn good_cart() -> Cart {
    let value = json!({
        "schema": 1,
        "id": "kanto-hard",
        "title": "Kanto Hard Mode",
        "version": "1.0.0",
        "author": "someone",
        "repo": "someone/kanto-hard",
        "summary": "Every trainer bites back.",
        "shell": "#8b1a1a",
        "label": "label.png",
        "base": "red",
        "engine": ">=1.0.0 <2.0.0",
        "seal": "sealed",
        "mods": [
            {"id": "harder-trainers", "source": "github",
             "repo": "someone/harder-trainers", "version": "2.1.0",
             "sha256": "a".repeat(64),
             "options": {"difficulty": "brutal", "levelCap": 100,
                         "nuzlocke": true}},
            {"id": "new-music", "source": "gamebanana", "mod": 546899,
             "file": 1294214, "md5": "b".repeat(32)}
        ],
        "load_order": ["new-music", "harder-trainers"]
    });
    value.as_object().expect("object").clone()
}

fn rules(findings: &[Finding]) -> Vec<String> {
    let mut rules: Vec<String> = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| f.rule.clone())
        .collect();
    rules.sort();
    rules.dedup();
    rules
}

fn errors_for(cart: &Cart) -> Vec<String> {
    rules(&schema_findings(cart, None))
}

#[test]
fn schema_accepts_a_full_cart() {
    assert!(schema_findings(&good_cart(), None).is_empty());
}

#[test]
fn identity_fields_are_checked() {
    let cases: Vec<(&str, Value)> = vec![
        ("id", json!("no spaces here")),
        ("id", json!("x".repeat(65))),
        ("title", json!("")),
        ("title", json!("t".repeat(49))),
        ("version", json!("1.0")),
        ("version", json!("v1.0.0")),
        ("author", json!("")),
        ("shell", json!("8b1a1a")),
        ("shell", json!("#8b1a1")),
        ("base", json!("nonesuch")),
        ("seal", json!("welded")),
        ("schema", json!(2)),
        ("summary", json!("s".repeat(121))),
        ("repo", json!("someone")),
        ("engine", json!(">=1.0.0 <<2.0.0")),
    ];
    for (key, value) in cases {
        let mut cart = good_cart();
        cart.insert(key.into(), value.clone());
        assert!(
            errors_for(&cart).contains(&"CK002".to_string()),
            "{} = {}",
            key,
            value
        );
    }
}

#[test]
fn optional_fields_stay_optional() {
    for key in ["repo", "summary", "engine", "label", "load_order", "seal"] {
        let mut cart = good_cart();
        cart.remove(key);
        assert!(schema_findings(&cart, None).is_empty(), "dropping {}", key);
    }
}

#[test]
fn unknown_top_level_fields_warn() {
    let mut cart = good_cart();
    cart.insert("colour".into(), json!("red"));
    let found = schema_findings(&cart, None);
    assert!(found
        .iter()
        .any(|f| f.rule == "CK001" && f.severity == Severity::Warn));
    assert!(rules(&found).is_empty());
}

#[test]
fn label_paths_cannot_escape() {
    for value in [
        "/etc/passwd",
        "../out.png",
        "a/../../b.png",
        &"x".repeat(129),
        "C:\\art.png",
    ] {
        let mut cart = good_cart();
        cart.insert("label".into(), json!(value));
        assert!(
            errors_for(&cart).contains(&"CK003".to_string()),
            "{}",
            value
        );
    }
}

#[test]
fn mods_list_bounds() {
    let mut cart = good_cart();
    cart.insert("mods".into(), json!([]));
    assert!(errors_for(&cart).contains(&"CK004".to_string()));

    let mut cart = good_cart();
    let first = cart["mods"][0].clone();
    cart.insert("mods".into(), json!(vec![first; 65]));
    assert!(errors_for(&cart).contains(&"CK004".to_string()));

    let mut cart = good_cart();
    cart["mods"][1] = cart["mods"][0].clone();
    cart.insert(
        "load_order".into(),
        json!(["harder-trainers", "harder-trainers"]),
    );
    assert!(errors_for(&cart).contains(&"CK004".to_string()));
}

#[test]
fn pin_fields_per_source() {
    let github: Vec<Value> = vec![
        json!({"source": "torrent"}),
        json!({"repo": "nope"}),
        json!({"version": "1.0"}),
        json!({"sha256": "A".repeat(64)}),
        json!({"sha256": "a".repeat(63)}),
    ];
    for patch in github {
        let mut cart = good_cart();
        for (key, value) in patch.as_object().expect("patch") {
            cart["mods"][0][key] = value.clone();
        }
        assert!(
            errors_for(&cart).contains(&"CK004".to_string()),
            "{}",
            patch
        );
    }
    let gamebanana: Vec<Value> = vec![
        json!({"mod": 0}),
        json!({"mod": "546899"}),
        json!({"file": -1}),
        json!({"md5": "B".repeat(32)}),
        json!({"md5": "b".repeat(31)}),
    ];
    for patch in gamebanana {
        let mut cart = good_cart();
        for (key, value) in patch.as_object().expect("patch") {
            cart["mods"][1][key] = value.clone();
        }
        assert!(
            errors_for(&cart).contains(&"CK004".to_string()),
            "{}",
            patch
        );
    }
}

#[test]
fn scaffold_placeholders_warn() {
    let mut cart = good_cart();
    cart["mods"][0]["repo"] = json!("owner/example-mod");
    cart["mods"][0]["sha256"] = json!("0".repeat(64));
    let found = schema_findings(&cart, None);
    assert!(rules(&found).is_empty());
    assert_eq!(
        found
            .iter()
            .filter(|f| f.severity == Severity::Warn)
            .count(),
        2
    );
}

#[test]
fn frozen_options_are_scalars() {
    let cases: Vec<Value> = vec![
        json!({"a": {"nested": 1}}),
        json!({"a": null}),
        json!({"a": ["list"]}),
        json!({"k".repeat(65): 1}),
        json!({"a": "x".repeat(257)}),
        Value::Object(
            (0..65)
                .map(|n| (n.to_string(), json!(n)))
                .collect::<serde_json::Map<String, Value>>(),
        ),
    ];
    for value in cases {
        let mut cart = good_cart();
        cart["mods"][0]["options"] = value.clone();
        assert!(
            errors_for(&cart).contains(&"CK004".to_string()),
            "{}",
            value
        );
    }
}

#[test]
fn load_order_is_a_permutation() {
    let cases: Vec<Value> = vec![
        json!(["harder-trainers"]),
        json!(["harder-trainers", "ghost"]),
        json!(["harder-trainers", "harder-trainers"]),
        json!("harder-trainers"),
        json!([1, 2]),
    ];
    for value in cases {
        let mut cart = good_cart();
        cart.insert("load_order".into(), value.clone());
        assert!(
            errors_for(&cart).contains(&"CK005".to_string()),
            "{}",
            value
        );
    }
}

#[test]
fn lua_string_escapes() {
    assert_eq!(lua_string("a\"b"), "\"a\\\"b\"");
    assert_eq!(lua_string("a\\b"), "\"a\\\\b\"");
    assert_eq!(lua_string("a\nb"), "\"a\\\nb\"");
    assert_eq!(lua_string("a\tb"), "\"a\\9b\"");
    assert_eq!(lua_string("a\t1"), "\"a\\0091\"");
    assert_eq!(lua_string("a\rb"), "\"a\\13b\"");
    assert_eq!(lua_string("Pokémon"), "\"Pokémon\"");
}

#[test]
fn lua_scalars_and_empty_tables() {
    let encode = |value: Value| lua_value(&value, 0).expect("encodes");
    assert_eq!(encode(json!(3)), "3");
    assert_eq!(encode(json!(-2)), "-2");
    assert_eq!(encode(json!(0.5)), "0.5");
    assert_eq!(encode(json!(true)), "true");
    assert_eq!(encode(json!([])), "{}");
    assert_eq!(encode(json!({})), "{}");
}

#[test]
fn bundle_shape_and_key_order() {
    let body = lua_encode(&bundle_table(&good_cart(), None, None)).expect("encodes");
    assert!(body.starts_with("return {\n"));
    assert!(body.ends_with("}\n"));
    assert!(body.contains("  format = \"g1rcart\","));
    assert!(body.contains("  formatVersion = 1,\n"));
    let cart_at = body.find("cart =").expect("cart");
    let format_at = body.find("format =").expect("format");
    let version_at = body.find("formatVersion =").expect("formatVersion");
    assert!(cart_at < format_at && format_at < version_at);
    assert!(body.contains("[1] = {"));
}

#[test]
fn bundle_materializes_defaults() {
    let mut cart = good_cart();
    cart.remove("seal");
    cart.remove("load_order");
    let packed = packed_cart(&cart);
    assert_eq!(packed["seal"], json!("sealed"));
    assert_eq!(
        packed["load_order"],
        json!(["harder-trainers", "new-music"])
    );
}

#[test]
fn bundle_is_order_independent() {
    let cart = good_cart();
    let mut shuffled = serde_json::Map::new();
    for key in cart.keys().rev() {
        shuffled.insert(key.clone(), cart[key].clone());
    }
    assert_eq!(
        lua_encode(&bundle_table(&cart, None, None)).expect("encodes"),
        lua_encode(&bundle_table(&shuffled, None, None)).expect("encodes")
    );
}

#[test]
fn bundle_carries_the_label_art() {
    use base64::Engine as _;
    let art = label_art("#8b1a1a");
    let table = bundle_table(&good_cart(), Some(&art), Some("label.png"));
    assert_eq!(table["labelArt"]["bytes"], json!(art.len()));
    let data = table["labelArt"]["data"].as_str().expect("data");
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .expect("decodes"),
        art
    );
}

#[test]
fn pin_specs() {
    let github = |slug: &str, version: &str| {
        Some(Spec::Github {
            slug: slug.into(),
            version: version.into(),
        })
    };
    assert_eq!(
        parse_spec("owner/repo@1.2.3"),
        github("owner/repo", "1.2.3")
    );
    assert_eq!(
        parse_spec("https://github.com/owner/repo@1.2.3"),
        github("owner/repo", "1.2.3")
    );
    assert_eq!(
        parse_spec("owner/repo.git@v1.2.3"),
        github("owner/repo", "1.2.3")
    );
    assert_eq!(
        parse_spec("https://gamebanana.com/mods/546899"),
        Some(Spec::GameBanana { mod_id: 546899 })
    );
    assert_eq!(
        parse_spec("gamebanana:546899"),
        Some(Spec::GameBanana { mod_id: 546899 })
    );
    assert_eq!(
        parse_spec("546899"),
        Some(Spec::GameBanana { mod_id: 546899 })
    );
    assert_eq!(parse_spec("not a spec"), None);
}

#[test]
fn option_parsing() {
    assert_eq!(
        parse_option("a=true").expect("parses"),
        ("a".into(), json!(true))
    );
    assert_eq!(parse_option("a=3").expect("parses"), ("a".into(), json!(3)));
    assert_eq!(
        parse_option("a=1.5").expect("parses"),
        ("a".into(), json!(1.5))
    );
    assert_eq!(
        parse_option("a=hard").expect("parses"),
        ("a".into(), json!("hard"))
    );
    assert!(parse_option("nope").is_err());
}

#[test]
fn engine_ranges() {
    for text in [">=1.0.0 <2.0.0", "^1.2", "1.2.3", ">1 || <0.9", "<=2"] {
        assert_eq!(range_problem(Some(&json!(text))), None, "{}", text);
    }
    for text in ["", ">=x", ">>1.0.0", "1.0.0 || "] {
        assert!(range_problem(Some(&json!(text))).is_some(), "{}", text);
    }
}

#[test]
fn label_png_is_deterministic() {
    let art = label_art("#123456");
    assert!(art.starts_with(&cartcore::schema::PNG_SIGNATURE));
    assert_eq!(&art[12..16], b"IHDR");
    assert!(art.ends_with(b"IEND\xae\x42\x60\x82"));
    assert_eq!(label_art("#123456"), art);
}
