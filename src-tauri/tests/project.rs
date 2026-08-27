//! Project directory behavior: round trip, pin edits, export refusal, label guard.

use g1r_cart_maker_lib::project;
use g1r_cart_maker_lib::settings::Settings;
use serde_json::{json, Value};
use std::path::Path;

fn new_project(dir: &Path) -> project::ProjectState {
    let request = project::ScaffoldRequest {
        parent: dir.to_string_lossy().to_string(),
        id: "demo_cart".into(),
        title: Some("Demo Cart".into()),
        author: Some("someone".into()),
        summary: Some("A demo cart.".into()),
        base: "red".into(),
        shell: Some("#8b1a1a".into()),
        seal: "sealed".into(),
        github: None,
        force: false,
    };
    let mut settings = Settings::default();
    project::scaffold(&request, &mut settings).expect("scaffold")
}

fn real_pin() -> Value {
    json!({
        "id": "harder-trainers",
        "source": "github",
        "repo": "someone/harder-trainers",
        "version": "2.1.0",
        "sha256": "c".repeat(64),
    })
}

#[test]
fn scaffold_open_save_round_trip() {
    let temp = tempdir::TempDir::new("project").expect("temp");
    let state = new_project(temp.path());
    let dir = Path::new(&state.dir);
    assert!(dir.join("cart.json").is_file());
    assert!(dir.join("label.png").is_file());
    assert!(state.has_workflow);
    assert!(state.label.exists);
    assert_eq!(state.label.width, Some(96));
    assert!(state
        .label
        .data_url
        .unwrap()
        .starts_with("data:image/png;base64,"));

    let mut cart = match state.cart {
        Value::Object(map) => map,
        _ => panic!("cart is an object"),
    };
    cart.insert("title".into(), json!("Renamed"));
    cart.insert("keepme".into(), json!({"nested": true}));
    let saved = project::save(dir, Value::Object(cart)).expect("save");
    let reopened = project::state(dir).expect("reopen");
    assert_eq!(saved.cart, reopened.cart);
    assert_eq!(reopened.cart["title"], json!("Renamed"));
    assert_eq!(reopened.cart["keepme"], json!({"nested": true}));
    let keys: Vec<&String> = reopened.cart.as_object().expect("object").keys().collect();
    assert_eq!(keys[0], "schema", "documented keys keep cartkit's order");
    assert_eq!(keys.last().expect("last"), &"keepme");
}

#[test]
fn adding_a_pin_evicts_the_placeholder() {
    let temp = tempdir::TempDir::new("project").expect("temp");
    let dir = Path::new(&new_project(temp.path()).dir).to_path_buf();

    let state = project::add_pin(&dir, real_pin()).expect("add");
    let mods = state.cart["mods"].as_array().expect("mods");
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0]["id"], json!("harder-trainers"));
    assert!(state.report.ok(true), "a real pin validates strictly");

    let state = project::set_pin_options(&dir, "harder-trainers", json!({"difficulty": "brutal"}))
        .expect("options");
    assert_eq!(
        state.cart["mods"][0]["options"]["difficulty"],
        json!("brutal")
    );
    let state = project::set_pin_enabled(&dir, "harder-trainers", false).expect("disable");
    assert_eq!(state.cart["mods"][0]["enabled"], json!(false));
    let state = project::set_pin_enabled(&dir, "harder-trainers", true).expect("enable");
    assert!(state.cart["mods"][0].get("enabled").is_none());

    let state = project::remove_pin(&dir, "harder-trainers").expect("remove");
    assert!(state.cart["mods"].as_array().expect("mods").is_empty());
}

#[test]
fn reordering_writes_a_permutation() {
    let temp = tempdir::TempDir::new("project").expect("temp");
    let dir = Path::new(&new_project(temp.path()).dir).to_path_buf();
    project::add_pin(&dir, real_pin()).expect("add");
    project::add_pin(
        &dir,
        json!({"id": "new-music", "source": "gamebanana", "mod": 546899,
               "file": 1294214, "md5": "b".repeat(32)}),
    )
    .expect("add");

    let state = project::reorder_pins(&dir, vec!["new-music".into(), "harder-trainers".into()])
        .expect("reorder");
    assert_eq!(
        state.cart["load_order"],
        json!(["new-music", "harder-trainers"])
    );
    assert!(state.report.ok(true));

    assert!(project::reorder_pins(&dir, vec!["ghost".into()]).is_err());
    assert!(project::reorder_pins(&dir, vec!["new-music".into()]).is_err());
}

#[test]
fn export_refuses_a_warning_and_writes_a_bundle_when_clean() {
    let temp = tempdir::TempDir::new("project").expect("temp");
    let dir = Path::new(&new_project(temp.path()).dir).to_path_buf();
    let out = temp.path().join("out.g1rcart");

    let refused = project::export_bundle(&dir, &out).expect_err("the placeholder pin warns");
    assert!(refused.detail.contains("CK004"));
    assert!(!out.exists());

    project::add_pin(&dir, real_pin()).expect("add");
    let exported = project::export_bundle(&dir, &out).expect("export");
    let body = std::fs::read(&out).expect("bundle");
    assert_eq!(exported.bytes, body.len() as u64);
    assert!(body.starts_with(b"return {\n"));
    assert!(body.ends_with(b"}\n"));
    let text = String::from_utf8(body).expect("utf-8");
    assert!(text.contains("id = \"demo_cart\""));
    assert!(text.contains("labelArt"));

    assert_eq!(
        project::default_bundle_name(&dir).expect("name"),
        "demo_cart-0.1.0.g1rcart"
    );
}

#[test]
fn label_writing_refuses_anything_the_manifest_would_reject() {
    let temp = tempdir::TempDir::new("project").expect("temp");
    let dir = Path::new(&new_project(temp.path()).dir).to_path_buf();

    let art = cartcore::labelart::label_art("#2f6f4f");
    let data_url = project::png_data_url(&art);
    let check = g1r_cart_maker_lib::label::write_png(&dir, "label.png", &data_url).expect("write");
    assert!(check.ok);
    assert_eq!(std::fs::read(dir.join("label.png")).expect("art"), art);

    for bad in ["../escape.png", "/tmp/escape.png", "sub dir/art.png"] {
        assert!(
            g1r_cart_maker_lib::label::write_png(&dir, bad, &data_url).is_err(),
            "{} must be refused",
            bad
        );
    }
    assert!(
        g1r_cart_maker_lib::label::write_png(&dir, "label.png", "data:image/jpeg;base64,AAAA")
            .is_err()
    );
    assert!(project::decode_png_data_url("data:image/png;base64,not-base64!!").is_err());
}

#[test]
fn label_documents_round_trip() {
    let temp = tempdir::TempDir::new("project").expect("temp");
    let dir = Path::new(&new_project(temp.path()).dir).to_path_buf();
    assert!(project::read_label_doc(&dir).is_none());

    let doc = cartcore::labeldoc::LabelDoc {
        template: "red".into(),
        background: "#101010".into(),
        layers: vec![cartcore::labeldoc::Layer {
            id: "title".into(),
            name: "Title".into(),
            x: 10.0,
            y: 20.0,
            width: 300.0,
            height: 60.0,
            rotation: 0.0,
            hidden: false,
            locked: false,
            from_template: Some("red".into()),
            body: cartcore::labeldoc::LayerBody::Text {
                text: "Demo Cart".into(),
                font: "system-ui".into(),
                size: 28.0,
                colour: "#ffffff".into(),
                align: cartcore::labeldoc::TextAlign::Center,
                weight: Some("700".into()),
                letter_spacing: None,
                line_height: None,
                stroke: None,
                stroke_width: None,
            },
        }],
        ..Default::default()
    };
    project::write_label_doc(&dir, &doc).expect("write doc");
    let read = project::read_label_doc(&dir).expect("doc");
    assert_eq!(read, doc);

    let newer = r##"{"schema":99,"width":500,"height":441,"template":"red","background":"#fff","layers":[]}"##;
    assert!(cartcore::labeldoc::parse_doc(newer).is_err());
}

#[test]
fn opening_a_directory_without_a_cart_says_so() {
    let temp = tempdir::TempDir::new("project").expect("temp");
    let mut settings = Settings::default();
    let problem = project::open(temp.path(), &mut settings).expect_err("no cart");
    assert_eq!(problem.kind, "not_found");
    assert!(problem.message.contains("cart.json"));
}

/// The Publish screen calls this on mount; a failure here is what the window
/// would show instead of the page.
#[test]
fn readiness_on_a_fresh_cart_does_not_blow_up() {
    let dir = tempdir::TempDir::new("readiness").unwrap();
    let project = new_project(dir.path());
    let report =
        g1r_cart_maker_lib::publishing::index_readiness(std::path::Path::new(&project.dir))
            .expect("readiness must answer for a cart with no repo");
    assert!(!report.items.is_empty());
}

/// The same, once a repo is set: this is the path that shells out to gh.
#[test]
fn readiness_with_a_repo_set_does_not_blow_up() {
    let dir = tempdir::TempDir::new("readiness-repo").unwrap();
    let project = new_project(dir.path());
    let path = std::path::Path::new(&project.dir).join("cart.json");
    let mut cart: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    cart["repo"] = Value::String("someone/demo_cart".into());
    std::fs::write(&path, serde_json::to_string_pretty(&cart).unwrap()).unwrap();
    let report =
        g1r_cart_maker_lib::publishing::index_readiness(std::path::Path::new(&project.dir))
            .expect("readiness must answer even when gh cannot");
    assert!(!report.items.is_empty());
}
