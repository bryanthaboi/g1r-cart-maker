#![allow(dead_code)]

//! A cart directory that passes strict validation, plus the gh payloads the
//! fake runner replays.

use cartcore::cart::{write_cart, Cart};
use cartcore::scaffold::{scaffold_into, ScaffoldOptions};
use serde_json::json;
use std::path::Path;

pub const SHA: &str = "1111111111111111111111111111111111111111111111111111111111111111";

pub fn make_cart(dir: &Path) -> Cart {
    let mut options = ScaffoldOptions::new("night-run");
    options.author = Some("bryanthaboi".to_string());
    options.summary = Some("A short run through Kanto at night.".to_string());
    options.github = Some("bryanthaboi/night-run".to_string());
    options.engine = "1.2.0".to_string();
    options.force = true;
    let mut cart = scaffold_into(dir, &options).expect("scaffold");
    cart.insert(
        "mods".to_string(),
        json!([{
            "id": "night-mode",
            "source": "github",
            "repo": "bryanthaboi/night-mode",
            "version": "1.0.0",
            "sha256": SHA,
        }]),
    );
    cart.insert("version".to_string(), json!("1.0.0"));
    write_cart(dir, &cart).expect("write cart");
    let report = cartcore::validate_cart(&cart, Some(dir));
    assert!(
        report.ok(true),
        "fixture cart must validate strictly: {:?}",
        report.findings
    );
    cart
}

pub fn run_list(tag: &str, id: i64) -> String {
    json!([{
        "databaseId": id,
        "status": "in_progress",
        "conclusion": null,
        "headBranch": tag,
        "event": "push",
        "name": "release",
    }])
    .to_string()
}

pub fn run_view(status: &str, conclusion: &str) -> String {
    json!({
        "status": status,
        "conclusion": conclusion,
        "url": "https://github.com/bryanthaboi/night-run/actions/runs/42",
        "jobs": [{
            "name": "release",
            "status": status,
            "conclusion": conclusion,
            "steps": [
                { "name": "cartkit selftest", "status": "completed", "conclusion": "success" },
                { "name": "validate --online", "status": status, "conclusion": conclusion },
            ],
        }],
    })
    .to_string()
}

pub fn release_view(tag: &str, assets: &[&str]) -> String {
    json!({
        "tagName": tag,
        "url": format!("https://github.com/bryanthaboi/night-run/releases/tag/{}", tag),
        "assets": assets.iter().map(|name| json!({ "name": name })).collect::<Vec<_>>(),
    })
    .to_string()
}
