use cartcore::modmanifest::{
    analyze, parse_dependency_specs, parse_github, parse_manifest, DependencySpec, Issue,
    ModManifest, PinnedMod,
};
use serde_json::{json, Value};
use std::path::PathBuf;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mods")
}

fn load(name: &str) -> ModManifest {
    let text =
        std::fs::read_to_string(fixtures().join(name).join("manifest.json")).expect("fixture");
    parse_manifest(&text)
        .unwrap_or_else(|err| panic!("{} should parse: {}", name, err))
        .manifest
}

fn specs(json: Value) -> Vec<DependencySpec> {
    parse_dependency_specs(Some(&json), "dependencies", None).expect("specs")
}

fn pin(id: &str, version: &str, deps: &[&str], conflicts: &[&str]) -> PinnedMod {
    PinnedMod {
        id: id.to_string(),
        version: version.to_string(),
        dependencies: specs(json!(deps)),
        optional_dependencies: Vec::new(),
        conflicts: specs(json!(conflicts)),
    }
}

// ------- manifest

#[test]
fn a_full_manifest_keeps_every_load_bearing_field() {
    let m = load("example_mod");
    assert_eq!(m.id, "example");
    assert_eq!(m.name, "Example Mod");
    assert_eq!(m.version, "1.2.0");
    assert_eq!(m.entry, "main.lua", "the leading ./ is normalized away");
    assert_eq!(m.api, 2);
    assert_eq!(m.priority, 5.0);
    assert_eq!(m.category, "GAMEPLAY");
    assert_eq!(m.profile, "overhaul");
    assert_eq!(m.permissions, vec!["network", "steps"]);
    assert_eq!(m.game_version.as_deref(), Some(">=1.0.0"));
    assert_eq!(m.github.as_deref(), Some("owner/example-mod"));
    assert_eq!(m.log_url.as_deref(), Some("https://example.invalid/logs"));
    assert_eq!(m.force_enable_env.as_deref(), Some("EXAMPLE_FORCE"));
    assert_eq!(m.options_schema.as_deref(), Some("options_schema.lua"));
    assert_eq!(m.assets_transforms.as_deref(), Some("transforms.lua"));
    assert!(m.experimental);
    assert!(!m.language);
    assert_eq!(m.games, vec!["red", "blue", "yellow", "crystal"]);
    assert!(m.gen2compat);
    assert!(
        m.affects_link,
        "an overhaul is assumed to move the fingerprint"
    );

    assert_eq!(
        m.dependencies,
        vec![
            DependencySpec {
                id: "base".into(),
                range: Some("^1.0.0".into()),
                github: None,
                games: None,
                game_version: None
            },
            DependencySpec {
                id: "helper".into(),
                range: Some(">=2.0.0".into()),
                github: Some("owner/helper".into()),
                games: None,
                game_version: None
            },
        ]
    );
    assert_eq!(m.optional_dependencies[0].id, "extra");
    assert_eq!(
        m.optional_dependencies[0].github.as_deref(),
        Some("owner/extra")
    );
    assert_eq!(
        m.conflicts
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        vec!["rival", "other"],
        "conflicts and incompatible merge, first wins"
    );
}

#[test]
fn defaults_match_the_engine() {
    let m = parse_manifest(r#"{"id":"bare","name":"Bare","version":"1.0.0","entry":"main.lua"}"#)
        .expect("bare manifest")
        .manifest;
    assert_eq!(m.api, 1);
    assert_eq!(m.priority, 0.0);
    assert_eq!(m.profile, "content");
    assert_eq!(m.category, "OTHER");
    assert_eq!(m.description, "");
    assert!(m.permissions.is_empty());
    assert!(!m.experimental);
    assert!(!m.gen2compat);
    assert!(!m.affects_link, "a content pack is not a link claim");
    assert_eq!(m.games, vec!["red", "blue", "yellow"]);

    let language = parse_manifest(
        r#"{"id":"es","name":"Spanish","version":"1.0.0","entry":"main.lua",
            "profile":"total_conversion","language":true}"#,
    )
    .unwrap()
    .manifest;
    assert!(
        !language.affects_link,
        "a declared translation is not a claim"
    );

    let explicit = parse_manifest(
        r#"{"id":"x","name":"X","version":"1.0.0","entry":"main.lua",
            "profile":"overhaul","affects_link":false}"#,
    )
    .unwrap()
    .manifest;
    assert!(!explicit.affects_link);
}

#[test]
fn api_1_warns_where_api_2_fails() {
    let parsed = parse_manifest(
        &std::fs::read_to_string(fixtures().join("legacy_mod/manifest.json")).unwrap(),
    )
    .expect("api 1 keeps loading");
    assert_eq!(parsed.warnings.len(), 2, "{:?}", parsed.warnings);
    assert!(parsed.warnings[0].contains("unknown profile"));
    assert!(parsed.warnings[1].contains("unknown permission"));
    assert_eq!(parsed.manifest.profile, "content");
    assert_eq!(parsed.manifest.permissions, vec!["network"]);
    assert!(
        parsed.manifest.log_url.is_none(),
        "api 1 never carries log_url"
    );
    assert!(parsed.manifest.gen2compat);
    assert_eq!(
        parsed
            .manifest
            .dependencies
            .iter()
            .map(|d| d.id.as_str())
            .collect::<Vec<_>>(),
        vec!["base", "helper"],
        "a dependency map normalizes to specs"
    );

    let strict = parse_manifest(
        r#"{"id":"x","name":"X","version":"1.0.0","entry":"main.lua","api":2,
            "profile":"sidequest"}"#,
    );
    assert!(strict.unwrap_err().contains("unknown profile"));
}

#[test]
fn log_url_needs_https_and_the_network_permission() {
    let missing = parse_manifest(
        r#"{"id":"x","name":"X","version":"1.0.0","entry":"main.lua","api":2,
            "log_url":"https://ok.invalid"}"#,
    );
    assert!(missing.unwrap_err().contains("network permission"));

    let insecure = parse_manifest(
        r#"{"id":"x","name":"X","version":"1.0.0","entry":"main.lua","api":2,
            "permissions":["network"],"log_url":"http://no.invalid"}"#,
    );
    assert!(insecure.unwrap_err().contains("https://"));
}

#[test]
fn paths_stay_inside_the_mod() {
    let err = parse_manifest(
        &std::fs::read_to_string(fixtures().join("broken_mod/manifest.json")).unwrap(),
    )
    .unwrap_err();
    assert!(err.contains("entry must stay inside"), "{}", err);

    for bad in ["/etc/passwd", "..\\win.ini", "C:/x.lua", "../up.lua"] {
        let text = format!(
            r#"{{"id":"x","name":"X","version":"1.0.0","entry":"main.lua","options_schema":"{}"}}"#,
            bad.replace('\\', "\\\\")
        );
        assert!(parse_manifest(&text).is_err(), "{} was accepted", bad);
    }
}

#[test]
fn malformed_manifests_are_errors_not_panics() {
    for text in [
        "",
        "[]",
        "null",
        r#"{"name":"No id","version":"1.0.0","entry":"main.lua"}"#,
        r#"{"id":"bad id","name":"X","version":"1.0.0","entry":"main.lua"}"#,
        r#"{"id":"x","name":"","version":"1.0.0","entry":"main.lua"}"#,
        r#"{"id":"x","name":"X","entry":"main.lua"}"#,
        r#"{"id":"x","name":"X","version":"1.0.0"}"#,
        r#"{"id":"x","name":"X","version":"1.0.0","entry":"main.lua","api":0}"#,
        r#"{"id":"x","name":"X","version":"1.0.0","entry":"main.lua","game_version":"=>1"}"#,
        r#"{"id":"x","name":"X","version":"1.0.0","entry":"main.lua","github":"http://evil.invalid/o/r"}"#,
        r#"{"id":"x","name":"X","version":"1.0.0","entry":"main.lua","permissions":"network"}"#,
        r#"{"id":"x","name":"X","version":"1.0.0","entry":"main.lua","experimental":"yes"}"#,
        r#"{"id":"x","name":"X","version":"1.0.0","entry":"main.lua","dependencies":["a@!!"]}"#,
        r#"{"id":"x","name":"X","version":"1.0.0","entry":"main.lua","dependencies":[42]}"#,
    ] {
        assert!(parse_manifest(text).is_err(), "accepted: {}", text);
    }
}

#[test]
fn imports_carry_their_digests_and_limits() {
    let m = load("imports_mod");
    let required = &m.required_imports[0];
    assert_eq!(required.id, "base_rom");
    assert_eq!(required.name, "Base ROM");
    assert_eq!(required.file, "base.gb");
    assert_eq!(required.md5, vec!["3d45c1ee9abd5738df46d8da13c4e14c"]);
    assert_eq!(required.format, "raw");
    assert_eq!(
        (required.size, required.max_size),
        (Some(1048576), Some(2097152))
    );
    assert!(required.required);

    let optional = &m.optional_imports[0];
    assert_eq!(optional.name, "bonus", "the name falls back to the id");
    assert_eq!(optional.description.as_deref(), Some("optional dump"));
    assert_eq!(optional.format, "n64");
    assert!(!optional.required);

    for bad in [
        r#"[{"id":"a","file":"roms/x.gb","md5":"3d45c1ee9abd5738df46d8da13c4e14c"}]"#,
        r#"[{"id":"a","file":".hidden","md5":"3d45c1ee9abd5738df46d8da13c4e14c"}]"#,
        r#"[{"id":"a","file":"x.gb","md5":"nothex"}]"#,
        r#"[{"id":"a","file":"x.gb","md5":[]}]"#,
        r#"[{"id":"a","file":"x.gb","md5":"3d45c1ee9abd5738df46d8da13c4e14c","format":"iso"}]"#,
        r#"[{"id":"a","file":"x.gb","md5":"3d45c1ee9abd5738df46d8da13c4e14c","size":0}]"#,
        r#"[{"id":"a","file":"x.gb","md5":"3d45c1ee9abd5738df46d8da13c4e14c","size":9,"max_size":4}]"#,
        r#"[{"id":"a","file":"x.gb","md5":"3d45c1ee9abd5738df46d8da13c4e14c"},
            {"id":"a","file":"y.gb","md5":"3d45c1ee9abd5738df46d8da13c4e14c"}]"#,
    ] {
        let text = format!(
            r#"{{"id":"x","name":"X","version":"1.0.0","entry":"main.lua","required_imports":{}}}"#,
            bad
        );
        assert!(parse_manifest(&text).is_err(), "accepted: {}", bad);
    }
}

#[test]
fn github_hints_are_normalized() {
    assert_eq!(
        parse_github("owner/repo").unwrap().as_deref(),
        Some("owner/repo")
    );
    assert_eq!(
        parse_github("https://github.com/owner/repo.git/")
            .unwrap()
            .as_deref(),
        Some("owner/repo")
    );
    assert_eq!(parse_github("  ").unwrap(), None);
    assert!(parse_github("owner/repo/extra").is_err());
    assert!(parse_github("not a repo").is_err());
}

// ------- dependency spec shapes

#[test]
fn every_dependency_shape_normalizes() {
    let m = load("deps_mod");
    assert_eq!(
        m.dependencies,
        vec![
            DependencySpec {
                id: "base".into(),
                range: None,
                github: Some("owner/base".into()),
                games: None,
                game_version: None
            },
            DependencySpec {
                id: "helper".into(),
                range: Some("^1.0.0".into()),
                github: None,
                games: None,
                game_version: None
            },
            DependencySpec {
                id: "themes".into(),
                range: Some(">=2.0.0 <3.0.0".into()),
                github: Some("owner/themes".into()),
                games: None,
                game_version: None
            },
            DependencySpec {
                id: "audio".into(),
                range: Some(">=1.4.0 <1.5.0".into()),
                github: Some("owner/audio".into()),
                games: Some(vec!["red".into(), "blue".into(), "yellow".into()]),
                game_version: Some(">=1.2.0".into())
            },
        ]
    );
    assert_eq!(m.conflicts[0].range.as_deref(), Some("<2.0.0"));

    let mapped = specs(json!({"base": "^1.0.0"}));
    assert_eq!(mapped[0].range.as_deref(), Some("^1.0.0"));
    assert_eq!(
        specs(json!(["solo#owner/solo"]))[0].github.as_deref(),
        Some("owner/solo")
    );
    assert_eq!(
        specs(json!([{"id":"a","version":"^2.0.0"}]))[0]
            .range
            .as_deref(),
        Some("^2.0.0")
    );
    assert!(parse_dependency_specs(Some(&json!("plain")), "dependencies", None).is_err());
    assert!(parse_dependency_specs(
        Some(&json!([{"id":"a","games":["nes"]}])),
        "dependencies",
        None
    )
    .is_err());
}

// ------- analysis

#[test]
fn a_missing_dependency_is_reported() {
    let issues = analyze(&[pin("a", "1.0.0", &["b@^1.0.0"], &[])]);
    assert_eq!(
        issues,
        vec![Issue::MissingDependency {
            mod_id: "a".into(),
            dependency: "b".into(),
            range: Some("^1.0.0".into())
        }]
    );
    assert!(issues[0].message().contains("not pinned"));
}

#[test]
fn a_dependency_outside_its_range_is_reported() {
    let issues = analyze(&[
        pin("a", "1.0.0", &["b@^2.0.0"], &[]),
        pin("b", "1.5.0", &[], &[]),
    ]);
    assert_eq!(
        issues,
        vec![Issue::UnsatisfiedDependency {
            mod_id: "a".into(),
            dependency: "b".into(),
            range: "^2.0.0".into(),
            pinned: "1.5.0".into(),
            optional: false
        }]
    );

    let satisfied = analyze(&[
        pin("a", "1.0.0", &["b@^1.0.0"], &[]),
        pin("b", "1.5.0", &[], &[]),
    ]);
    assert!(satisfied.is_empty());
}

#[test]
fn an_optional_dependency_is_only_judged_when_it_is_pinned() {
    let mut a = pin("a", "1.0.0", &[], &[]);
    a.optional_dependencies = specs(json!(["b@^2.0.0"]));
    assert!(analyze(std::slice::from_ref(&a)).is_empty());

    let issues = analyze(&[a, pin("b", "1.0.0", &[], &[])]);
    assert_eq!(issues.len(), 1);
    assert!(matches!(
        &issues[0],
        Issue::UnsatisfiedDependency { optional: true, .. }
    ));
}

#[test]
fn a_cycle_is_named_once() {
    let issues = analyze(&[
        pin("a", "1.0.0", &["b"], &[]),
        pin("b", "1.0.0", &["c"], &[]),
        pin("c", "1.0.0", &["a"], &[]),
    ]);
    assert_eq!(
        issues,
        vec![Issue::CircularDependency {
            cycle: vec!["a".into(), "b".into(), "c".into(), "a".into()]
        }]
    );
    assert!(issues[0].message().contains("a -> b -> c -> a"));

    let two = analyze(&[
        pin("a", "1.0.0", &["b"], &[]),
        pin("b", "1.0.0", &["a"], &[]),
    ]);
    assert_eq!(two.len(), 1);
    assert!(
        analyze(&[pin("a", "1.0.0", &["a"], &[])]).is_empty(),
        "self is not a chain"
    );
}

#[test]
fn a_declared_conflict_between_two_pins_is_reported_once() {
    let issues = analyze(&[
        pin("a", "1.0.0", &[], &["b"]),
        pin("b", "1.0.0", &[], &["a"]),
    ]);
    assert_eq!(
        issues,
        vec![Issue::Conflict {
            mod_id: "a".into(),
            conflicts_with: "b".into(),
            range: None
        }]
    );

    let ranged = analyze(&[
        pin("a", "1.0.0", &[], &["b@<2.0.0"]),
        pin("b", "2.5.0", &[], &[]),
    ]);
    assert!(
        ranged.is_empty(),
        "a conflict outside its range is not proven"
    );
    assert!(analyze(&[pin("a", "1.0.0", &[], &["ghost"])]).is_empty());
}

#[test]
fn analysis_runs_off_parsed_manifests() {
    let pins = vec![
        PinnedMod::from_manifest(&load("example_mod")),
        PinnedMod {
            id: "base".into(),
            version: "0.9.0".into(),
            dependencies: Vec::new(),
            optional_dependencies: Vec::new(),
            conflicts: Vec::new(),
        },
    ];
    let issues = analyze(&pins);
    assert_eq!(issues.len(), 2, "{:?}", issues);
    assert!(
        matches!(&issues[0], Issue::UnsatisfiedDependency { dependency, .. } if dependency == "base")
    );
    assert!(
        matches!(&issues[1], Issue::MissingDependency { dependency, .. } if dependency == "helper")
    );
}
