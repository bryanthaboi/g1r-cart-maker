mod support;

use cartcore::findings::Severity;
use resolve::cancel_flag;
use resolve::online::{online_findings_with, NO_TOKEN_NOTE};
use serde_json::{json, Value};
use std::sync::Arc;
use support::{Reply, TestServer};

const HELLO_SHA: &str = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
const OTHER_SHA: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const MD5_A: &str = "0123456789abcdef0123456789abcdef";
const MD5_B: &str = "fedcba9876543210fedcba9876543210";
const TAG_V: &str = "/repos/owner/demo/releases/tags/v1.2.3";
const GB_PAGE: &str = "/apiv11/Mod/4242/DownloadPage";

fn cart(mods: Value) -> cartcore::Cart {
    match json!({ "schema": 1, "id": "demo", "mods": mods }) {
        Value::Object(map) => map,
        _ => unreachable!(),
    }
}

fn github_cart(sha256: &str) -> cartcore::Cart {
    cart(json!([{
        "id": "demo",
        "source": "github",
        "repo": "owner/demo",
        "version": "1.2.3",
        "sha256": sha256,
    }]))
}

fn gb_cart(mod_id: Value, file: Value, md5: &str) -> cartcore::Cart {
    cart(json!([{
        "id": "banana",
        "source": "gamebanana",
        "mod": mod_id,
        "file": file,
        "md5": md5,
    }]))
}

fn release(base: &str, names: &[&str]) -> Value {
    let assets: Vec<Value> = names
        .iter()
        .map(|name| {
            json!({
                "name": name,
                "size": 11,
                "browser_download_url": format!("{}/dl/{}", base, name),
            })
        })
        .collect();
    json!({ "tag_name": "v1.2.3", "assets": assets })
}

fn rules(findings: &[cartcore::Finding]) -> Vec<&str> {
    findings.iter().map(|found| found.rule.as_str()).collect()
}

#[test]
fn a_matching_pin_is_silent() {
    let server = TestServer::start_based(|path, _hit, base| match path {
        TAG_V => Reply::ok(release(base, &["demo-1.2.3.zip"]).to_string()),
        "/dl/demo-1.2.3.zip" => Reply::ok("hello world"),
        _ => Reply::status(404),
    });
    let client = support::client(&server);
    let (findings, _) =
        online_findings_with(&client, &github_cart(HELLO_SHA), true, &cancel_flag());
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn ck100_when_the_release_is_not_there() {
    let server = TestServer::fixtures(vec![]);
    let client = support::client(&server);
    let (findings, notes) =
        online_findings_with(&client, &github_cart(HELLO_SHA), true, &cancel_flag());
    assert_eq!(rules(&findings), ["CK100"]);
    assert_eq!(findings[0].severity, Severity::Error);
    assert_eq!(
        findings[0].message,
        "mods[1] demo does not resolve: owner/demo has no release tagged v1.2.3 or 1.2.3"
    );
    assert!(notes.iter().all(|note| !note.contains("does not resolve")));
}

#[test]
fn ck101_when_the_hash_moved() {
    let server = TestServer::start_based(|path, _hit, base| match path {
        TAG_V => Reply::ok(release(base, &["demo-1.2.3.zip"]).to_string()),
        "/dl/demo-1.2.3.zip" => Reply::ok("hello world"),
        _ => Reply::status(404),
    });
    let client = support::client(&server);
    let (findings, _) =
        online_findings_with(&client, &github_cart(OTHER_SHA), true, &cancel_flag());
    assert_eq!(rules(&findings), ["CK101"]);
    assert_eq!(
        findings[0].message,
        format!(
            "mods[1] demo pins sha256 {} but demo-1.2.3.zip on owner/demo v1.2.3 hashes to {}; \
             re-pin it",
            OTHER_SHA, HELLO_SHA
        )
    );
}

#[test]
fn an_unreachable_api_is_a_note_and_never_a_finding() {
    let server = TestServer::start(Arc::new(|_path, _hit| Reply::status(500)));
    let client = support::client(&server);
    let (findings, notes) =
        online_findings_with(&client, &github_cart(HELLO_SHA), true, &cancel_flag());
    assert!(findings.is_empty(), "{findings:?}");
    assert!(
        notes
            .iter()
            .any(|note| note.starts_with("mods[1] demo not resolved: ")),
        "{notes:?}"
    );
}

/// The advice deliberately differs from cartkit's. cartkit is a CLI where
/// GITHUB_TOKEN is the only lever; the app has gh's own credential, so it says
/// so. The finding stays a note either way, which is the part that must match.
#[test]
fn a_rate_limit_is_a_note_pointing_at_gh() {
    let server = TestServer::start(Arc::new(|_path, _hit| {
        Reply::status(403).header("X-RateLimit-Remaining", "0")
    }));
    let client = support::client(&server);
    let (findings, notes) =
        online_findings_with(&client, &github_cart(HELLO_SHA), true, &cancel_flag());
    assert!(findings.is_empty(), "{findings:?}");
    assert!(
        notes.iter().any(|note| note.contains("gh auth login")),
        "{notes:?}"
    );
}

#[test]
fn no_download_notes_the_unchecked_hash() {
    let server = TestServer::start_based(|path, _hit, base| match path {
        TAG_V => Reply::ok(release(base, &["demo-1.2.3.zip"]).to_string()),
        _ => Reply::status(404),
    });
    let client = support::client(&server);
    let (findings, notes) =
        online_findings_with(&client, &github_cart(OTHER_SHA), false, &cancel_flag());
    assert!(findings.is_empty(), "{findings:?}");
    assert!(
        notes.iter().any(|note| note
            == "mods[1] demo hash not checked: the release publishes no sha256sums.txt and \
                --no-download was given"),
        "{notes:?}"
    );
}

#[test]
fn a_malformed_pin_is_left_to_the_offline_rules() {
    let server = TestServer::fixtures(vec![]);
    let client = support::client(&server);
    let bad = cart(json!([{
        "id": "demo", "source": "github", "repo": "not a slug", "version": "1.2.3",
        "sha256": HELLO_SHA,
    }]));
    let (findings, _) = online_findings_with(&client, &bad, true, &cancel_flag());
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(server.hits(TAG_V), 0);
}

fn gb_server(payload: Value) -> TestServer {
    TestServer::start(Arc::new(move |path: &str, _hit| {
        if path == GB_PAGE {
            Reply::ok(payload.to_string())
        } else {
            Reply::status(404)
        }
    }))
}

fn gb_file(id: u64, name: &str, md5: &str) -> Value {
    json!({ "_idRow": id, "_sFile": name, "_sMd5Checksum": md5 })
}

#[test]
fn a_matching_gamebanana_pin_is_silent() {
    let server = gb_server(json!({ "_aFiles": [gb_file(11, "a.zip", MD5_A)] }));
    let client = support::client(&server);
    let (findings, _) = online_findings_with(
        &client,
        &gb_cart(json!(4242), json!(11), MD5_A),
        true,
        &cancel_flag(),
    );
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn ck110_when_the_mod_is_withheld() {
    let server =
        gb_server(json!({ "_bIsWithheld": true, "_aFiles": [gb_file(11, "a.zip", MD5_A)] }));
    let client = support::client(&server);
    let (findings, _) = online_findings_with(
        &client,
        &gb_cart(json!(4242), json!(11), MD5_A),
        true,
        &cancel_flag(),
    );
    assert_eq!(rules(&findings), ["CK110"]);
    assert_eq!(
        findings[0].message,
        "mods[1] banana does not resolve: GameBanana mod 4242 is trashed or withheld"
    );
}

#[test]
fn ck110_when_the_file_is_not_on_the_mod() {
    let server = gb_server(json!({
        "_aFiles": [gb_file(11, "a.zip", MD5_A), gb_file(22, "b.zip", MD5_B)]
    }));
    let client = support::client(&server);
    let (findings, _) = online_findings_with(
        &client,
        &gb_cart(json!(4242), json!(99), MD5_A),
        true,
        &cancel_flag(),
    );
    assert_eq!(rules(&findings), ["CK110"]);
    assert_eq!(
        findings[0].message,
        "mods[1] banana pins file 99, which is not on GameBanana mod 4242 (it publishes 11, 22)"
    );
}

#[test]
fn ck111_when_the_md5_moved() {
    let server = gb_server(json!({ "_aFiles": [gb_file(11, "a.zip", MD5_B)] }));
    let client = support::client(&server);
    let (findings, _) = online_findings_with(
        &client,
        &gb_cart(json!(4242), json!(11), MD5_A),
        true,
        &cancel_flag(),
    );
    assert_eq!(rules(&findings), ["CK111"]);
    assert_eq!(
        findings[0].message,
        format!(
            "mods[1] banana pins md5 {} but file 11 (a.zip) publishes {}; re-pin it",
            MD5_A, MD5_B
        )
    );
}

#[test]
fn ck111_says_no_checksum_when_none_is_published() {
    let server = gb_server(json!({ "_aFiles": [gb_file(11, "a.zip", "")] }));
    let client = support::client(&server);
    let (findings, _) = online_findings_with(
        &client,
        &gb_cart(json!(4242), json!(11), MD5_A),
        true,
        &cancel_flag(),
    );
    assert_eq!(rules(&findings), ["CK111"]);
    assert!(findings[0]
        .message
        .ends_with("publishes no checksum; re-pin it"));
}

#[test]
fn an_unreachable_gamebanana_is_a_note() {
    let server = TestServer::start(Arc::new(|_path, _hit| Reply::status(502)));
    let client = support::client(&server);
    let (findings, notes) = online_findings_with(
        &client,
        &gb_cart(json!(4242), json!(11), MD5_A),
        true,
        &cancel_flag(),
    );
    assert!(findings.is_empty(), "{findings:?}");
    assert!(
        notes
            .iter()
            .any(|note| note.starts_with("mods[1] banana not resolved: ")),
        "{notes:?}"
    );
}

#[test]
fn a_non_integer_gamebanana_pin_is_skipped() {
    let server = TestServer::fixtures(vec![]);
    let client = support::client(&server);
    for (mod_id, file) in [
        (json!(0), json!(11)),
        (json!(4242), json!(-1)),
        (json!(true), json!(11)),
        (json!(4242.5), json!(11)),
        (json!("4242"), json!(11)),
    ] {
        let (findings, _) =
            online_findings_with(&client, &gb_cart(mod_id, file, MD5_A), true, &cancel_flag());
        assert!(findings.is_empty(), "{findings:?}");
    }
    assert_eq!(server.hits(GB_PAGE), 0);
}

#[test]
fn a_missing_token_is_noted() {
    let names = ["CARTKIT_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"];
    let saved: Vec<Option<String>> = names.iter().map(|name| std::env::var(name).ok()).collect();
    for name in names {
        std::env::remove_var(name);
    }

    let server = TestServer::fixtures(vec![]);
    let mut client = resolve::Client::new(None);
    client.set_api_base(&server.base);
    client.set_wait_scale(0.0);
    let (_, notes) = online_findings_with(&client, &cart(json!([])), true, &cancel_flag());
    assert_eq!(notes, [NO_TOKEN_NOTE]);

    for (name, value) in names.iter().zip(saved) {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}

#[test]
fn a_cancel_stops_the_pass() {
    let server = TestServer::fixtures(vec![]);
    let client = support::client(&server);
    let cancel = cancel_flag();
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    let (findings, notes) = online_findings_with(&client, &github_cart(HELLO_SHA), true, &cancel);
    assert!(findings.is_empty());
    assert!(
        notes.contains(&"online validation cancelled".to_string()),
        "{notes:?}"
    );
    assert_eq!(server.hits(TAG_V), 0);
}
