use cartcore::labeldoc::{canvas_for, LabelDoc};
use cartcore::modmanifest::{generation_of, layout_of, parse_manifest};
use cartcore::scaffold::{scaffold_cart, ScaffoldOptions};
use cartcore::schema::{cart_shape, is_base, BASES};
use serde_json::json;

const GEN3: [&str; 5] = ["firered", "leafgreen", "ruby", "sapphire", "emerald"];

#[test]
fn bases_follow_game_version_order() {
    assert_eq!(
        BASES,
        [
            "red",
            "blue",
            "yellow",
            "gold",
            "silver",
            "crystal",
            "firered",
            "leafgreen",
            "ruby",
            "sapphire",
            "emerald"
        ]
    );
    for base in GEN3 {
        assert!(is_base(base), "{}", base);
        assert_eq!(generation_of(base), 3, "{}", base);
        assert_eq!(cart_shape(base), "gba", "{}", base);
    }
    for base in ["red", "blue", "yellow", "gold", "silver", "crystal"] {
        assert_eq!(cart_shape(base), "gb", "{}", base);
    }
    assert_eq!(layout_of("firered"), Some("frlg"));
    assert_eq!(layout_of("sapphire"), Some("rse"));
    assert_eq!(layout_of("crystal"), None);
}

#[test]
fn a_gen3_cart_passes_identity_checks() {
    for base in GEN3 {
        let cart = json!({
            "schema": 1, "id": "demo", "title": "Demo", "version": "1.0.0",
            "author": "someone", "base": base, "seal": "sealed", "shell": "#dc3030",
            "mods": [{ "id": "m", "source": "github", "repo": "a/b", "version": "1.0.0",
                       "sha256": "0".repeat(64) }],
        });
        let cart = cart.as_object().unwrap().clone();
        let mut findings = Vec::new();
        cartcore::validate::check_identity(&cart, &mut findings);
        assert!(
            findings.iter().all(|f| !f.message.contains("base")),
            "{}: {:?}",
            base,
            findings
                .iter()
                .map(|f| f.message.clone())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn scaffold_accepts_gen3_bases() {
    for base in GEN3 {
        let options = ScaffoldOptions {
            base: base.into(),
            ..ScaffoldOptions::new("demo")
        };
        let cart = scaffold_cart(&options).expect("scaffold");
        assert_eq!(cart["base"], base);
    }
    let bad = ScaffoldOptions {
        base: "pinball".into(),
        ..ScaffoldOptions::new("demo")
    };
    let message = scaffold_cart(&bad).expect_err("unknown base").to_string();
    assert!(message.contains("sapphire"), "{}", message);
}

#[test]
fn gba_labels_use_the_launcher_canvas() {
    assert_eq!(canvas_for("emerald"), (512, 260));
    assert_eq!(canvas_for("red"), (500, 441));
    let doc = LabelDoc::for_base("leafgreen");
    assert_eq!((doc.width, doc.height), (512, 260));
    assert_eq!(doc.template, "blank");
}

#[test]
fn games_tokens_expand_to_gen3() {
    let parse = |games: &str| {
        parse_manifest(&format!(
            r#"{{"id":"x","name":"X","version":"1.0.0","entry":"main.lua","games":{}}}"#,
            games
        ))
        .expect("manifest")
        .manifest
        .games
    };
    assert_eq!(parse(r#"["gen3"]"#), GEN3.to_vec());
    assert_eq!(parse(r#"["frlg"]"#), vec!["firered", "leafgreen"]);
    assert_eq!(
        parse(r#"["rse", "red"]"#),
        vec!["red", "ruby", "sapphire", "emerald"]
    );
    assert_eq!(parse(r#"["all"]"#).len(), 11);
}

#[test]
fn legacy_gen2compat_stops_at_gen2() {
    let m = parse_manifest(
        r#"{"id":"x","name":"X","version":"1.0.0","entry":"main.lua","gen2compat":true}"#,
    )
    .expect("manifest")
    .manifest;
    assert_eq!(
        m.games,
        vec!["red", "blue", "yellow", "gold", "silver", "crystal"]
    );
}
