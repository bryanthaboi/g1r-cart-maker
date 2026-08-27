//! The window reads these shapes by name. A rename here is a runtime break, so
//! the keys are asserted rather than assumed.

use g1r_cart_maker_lib::dto;
use serde_json::Value;

fn keys(value: &Value) -> Vec<String> {
    value
        .as_object()
        .expect("object")
        .keys()
        .cloned()
        .collect::<Vec<_>>()
}

fn has(value: &Value, wanted: &[&str]) {
    let found = keys(value);
    for key in wanted {
        assert!(
            found.contains(&(*key).to_string()),
            "missing {} in {:?}",
            key,
            found
        );
    }
}

#[test]
fn install_instructions_shape() {
    let guide = toolchain::instructions::guide(
        toolchain::instructions::Tool::Gh,
        toolchain::instructions::Platform::MacOs,
    );
    let view = dto::InstallInstructions::from(&guide);
    let json = serde_json::to_value(&view).expect("serializes");
    has(&json, &["tool", "os", "title", "note", "notes", "steps"]);
    assert_eq!(json["tool"], "gh");
    let steps = json["steps"].as_array().expect("steps");
    assert!(!steps.is_empty());
    has(&steps[0], &["label", "command", "url"]);
}

#[test]
fn readiness_shape() {
    let cart = cartcore::cart::parse_cart(
        r##"{"schema":1,"id":"demo","title":"Demo","version":"1.0.0","author":"a",
            "shell":"#8b1a1a","base":"red","repo":"o/demo",
            "mods":[{"id":"m","source":"github","repo":"o/m","version":"1.0.0",
                     "sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]}"##,
    )
    .expect("cart");
    let hints = toolchain::readiness::IndexHints::from_cart(&cart);
    let readiness = toolchain::readiness::evaluate(&cart, None, &hints);
    let view = dto::ReadinessReport::from(&readiness);
    let json = serde_json::to_value(&view).expect("serializes");
    has(&json, &["items", "ready", "unknown"]);
    let items = json["items"].as_array().expect("items");
    assert!(!items.is_empty());
    has(
        &items[0],
        &[
            "id",
            "label",
            "ok",
            "blocking",
            "detail",
            "fix",
            "fixCommand",
        ],
    );
}

#[test]
fn findings_shape() {
    let cart = cartcore::cart::parse_cart(r#"{"id":"demo"}"#).expect("cart");
    let report = cartcore::validate_cart(&cart, None);
    let json = serde_json::to_value(&report).expect("serializes");
    has(&json, &["findings", "notes"]);
    let findings = json["findings"].as_array().expect("findings");
    has(&findings[0], &["rule", "severity", "message", "path"]);
    assert!(findings
        .iter()
        .all(|finding| finding["severity"] == "error" || finding["severity"] == "warn"));
}

#[test]
fn option_rows_shape() {
    let snapshot = r#"{"schema_version":1,"mods":{"m":[
        {"key":"a","type":"toggle","label":"A","default":true},
        {"key":"b","type":"choice","label":"B","default":"x","choices":[["X","x"]]},
        {"key":"c","type":"number","label":"C","default":1,"min":0,"max":9,"step":1},
        {"key":"d","type":"text","label":"D","default":"","maxLen":8,
         "visible_if":{"key":"a","equals":true}}]}}"#;
    let parsed = cartcore::optionschema::parse_snapshot(snapshot).expect("snapshot");
    let rows = parsed.get("m").expect("mod rows");
    let json = serde_json::to_value(rows).expect("serializes");
    let rows = json.as_array().expect("array");
    assert_eq!(rows.len(), 4);
    has(&rows[0], &["key", "label", "type", "default"]);
    assert_eq!(rows[1]["type"], "choice");
    has(&rows[1], &["choices"]);
    has(&rows[2], &["min", "max", "step"]);
    has(&rows[3], &["maxLen", "visible_if"]);
}

#[test]
fn label_template_and_export_shapes() {
    let templates = g1r_cart_maker_lib::label::templates();
    assert_eq!(templates.len(), 6);
    let json = serde_json::to_value(&templates[0]).expect("serializes");
    has(&json, &["id", "name", "base", "width", "height", "dataUrl"]);
    assert_eq!(json["width"], 500);
    assert_eq!(json["height"], 441);
    assert!(json["dataUrl"]
        .as_str()
        .expect("data url")
        .starts_with("data:image/png;base64,"));

    let art = cartcore::labelart::label_art("#8b1a1a");
    let check = cartcore::labeldoc::check_export(&art, "label.png");
    let json = serde_json::to_value(&check).expect("serializes");
    has(
        &json,
        &["ok", "bytes", "width", "height", "problems", "warnings"],
    );
}

#[test]
fn release_and_file_shapes() {
    let release = resolve::github::ReleaseSummary {
        tag: "v1.0.0".into(),
        name: Some("1.0.0".into()),
        published_at: Some("2026-01-01T00:00:00Z".into()),
        prerelease: false,
        assets: vec![resolve::github::Asset {
            name: "mod-1.0.0.zip".into(),
            size: 10,
            url: "https://example.invalid/a.zip".into(),
        }],
    };
    let json = serde_json::to_value(dto::Release::from(&release)).expect("serializes");
    has(
        &json,
        &["tag", "name", "publishedAt", "prerelease", "assets"],
    );
    has(&json["assets"][0], &["name", "size", "url"]);

    let file = resolve::gamebanana::GbFile {
        id: Some(7),
        file: "mod.zip".into(),
        filesize: 12,
        md5: "b".repeat(32),
        description: "".into(),
        download_count: 3,
        download_url: None,
    };
    let json = serde_json::to_value(dto::GameBananaFile::from(&file)).expect("serializes");
    has(
        &json,
        &["id", "file", "size", "md5", "description", "downloads"],
    );
}

#[test]
fn project_state_shape() {
    let temp = tempdir::TempDir::new("contract").expect("temp");
    let options = cartcore::scaffold::ScaffoldOptions::new("demo_cart");
    let dest = temp.path().join("demo_cart");
    cartcore::scaffold::scaffold_into(&dest, &options).expect("scaffold");
    let state = g1r_cart_maker_lib::project::state(&dest).expect("state");
    let json = serde_json::to_value(&state).expect("serializes");
    has(
        &json,
        &[
            "dir",
            "cart",
            "label",
            "labelDoc",
            "report",
            "hasWorkflow",
            "isGitRepo",
        ],
    );
    has(
        &json["label"],
        &["path", "exists", "bytes", "width", "height", "dataUrl"],
    );
}

/// The app must reach GitHub as the signed-in user without anyone setting
/// GITHUB_TOKEN. Ignored by default; run with
/// `cargo test -p g1r-cart-maker --test contract -- --ignored authenticated`.
#[test]
#[ignore]
fn authenticated_requests_use_ghs_own_credential() {
    let token = toolchain::detect::gh_token();
    assert!(token.is_some(), "gh must be signed in for this check");
    let client = resolve::http::Client::new(token.as_deref());
    let body: serde_json::Value = client
        .get_json("https://api.github.com/rate_limit", None, true)
        .expect("rate_limit must answer");
    let limit = body["resources"]["core"]["limit"].as_u64().unwrap_or(0);
    println!("core rate limit: {}", limit);
    assert!(
        limit > 60,
        "an anonymous caller gets 60/hour; got {}, so the credential was not used",
        limit
    );
}

/// Every path that reaches GitHub must carry the credential. Two call sites
/// silently passed None; this fails if a third appears.
#[test]
fn no_github_path_drops_the_credential() {
    let source = include_str!("../src/network.rs");
    for (number, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        assert!(
            !trimmed.starts_with("let _ = state"),
            "network.rs:{} ignores AppState, which is how the credential got \
             dropped before; take the token from state.github_token()",
            number + 1,
        );
    }
    assert!(
        source.contains("state.github_token().as_deref()"),
        "validate_online must pass the resolved credential",
    );
    assert!(
        source.contains("Client::new(state.github_token().as_deref())"),
        "client() must build with the resolved credential",
    );
}
