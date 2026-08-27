//! Label checks that need real files on disk.

use serde_json::json;

/// The trap that shipped a placeholder: the design saved, the art did not.
#[test]
fn a_design_beside_the_placeholder_is_a_warning() {
    let dir = tempdir::TempDir::new("stale-label").unwrap();
    let cart = json!({
        "schema": 1, "id": "demo", "title": "Demo", "version": "1.0.0",
        "author": "someone", "base": "red", "seal": "sealed", "label": "label.png",
        "mods": [{ "id": "m", "source": "github", "repo": "a/b", "version": "1.0.0",
                   "sha256": "0".repeat(64) }],
    });
    let cart = cart.as_object().unwrap().clone();
    std::fs::write(
        dir.path().join("label.png"),
        cartcore::labelart::label_art("#8b1a1a"),
    )
    .unwrap();
    std::fs::write(dir.path().join("label.layers.json"), "{}").unwrap();

    let mut findings = Vec::new();
    cartcore::validate::check_label(&cart, Some(dir.path()), &mut findings);
    let text: Vec<String> = findings.iter().map(|f| f.message.clone()).collect();
    assert!(
        text.iter().any(|m| m.contains("scaffold placeholder")),
        "{:?}",
        text
    );
}

/// Art older than the design would ship the wrong picture.
#[test]
fn art_older_than_the_design_is_a_warning() {
    let dir = tempdir::TempDir::new("older-label").unwrap();
    let cart = json!({
        "schema": 1, "id": "demo", "title": "Demo", "version": "1.0.0",
        "author": "someone", "base": "red", "seal": "sealed", "label": "label.png",
        "mods": [{ "id": "m", "source": "github", "repo": "a/b", "version": "1.0.0",
                   "sha256": "0".repeat(64) }],
    });
    let cart = cart.as_object().unwrap().clone();
    // a 500x441 PNG, so it is not the placeholder
    let rows: Vec<Vec<(i32, i32, i32)>> = (0..441).map(|_| vec![(17, 34, 51); 500]).collect();
    let art = cartcore::labelart::png_bytes(500, 441, &rows);
    std::fs::write(dir.path().join("label.png"), art).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(dir.path().join("label.layers.json"), "{}").unwrap();

    let mut findings = Vec::new();
    cartcore::validate::check_label(&cart, Some(dir.path()), &mut findings);
    let text: Vec<String> = findings.iter().map(|f| f.message.clone()).collect();
    assert!(
        text.iter().any(|m| m.contains("edited after")),
        "{:?}",
        text
    );
}
