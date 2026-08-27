mod support;

use resolve::gamebanana::{gamebanana_file, gamebanana_files, pin_gamebanana, GbPin};
use serde_json::{json, Map, Value};
use std::sync::Arc;
use support::{Reply, TestServer};

const PAGE: &str = "/apiv11/Mod/4242/DownloadPage";
const MD5_A: &str = "0123456789abcdef0123456789abcdef";
const MD5_B: &str = "fedcba9876543210fedcba9876543210";

fn file(id: u64, name: &str, md5: &str) -> Value {
    json!({
        "_idRow": id,
        "_sFile": name,
        "_nFilesize": 1234,
        "_sMd5Checksum": md5,
        "_sDescription": "the mod",
        "_nDownloadCount": 99,
    })
}

fn serve(payload: Value) -> TestServer {
    TestServer::start(Arc::new(move |path: &str, _hit| {
        if path == PAGE {
            Reply::ok(payload.to_string())
        } else {
            Reply::status(404)
        }
    }))
}

#[test]
fn the_file_list_carries_every_published_field() {
    let server = serve(json!({ "_aFiles": [file(11, "cool-mod.zip", MD5_A)] }));
    let client = support::client(&server);
    let files = gamebanana_files(&client, 4242).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].id, Some(11));
    assert_eq!(files[0].file, "cool-mod.zip");
    assert_eq!(files[0].filesize, 1234);
    assert_eq!(files[0].md5, MD5_A);
    assert_eq!(files[0].description, "the mod");
    assert_eq!(files[0].download_count, 99);
}

#[test]
fn a_file_is_looked_up_by_id() {
    let server = serve(json!({ "_aFiles": [file(11, "a.zip", MD5_A), file(22, "b.zip", MD5_B)] }));
    let client = support::client(&server);
    let files = gamebanana_files(&client, 4242).unwrap();
    assert_eq!(gamebanana_file(&files, 22).unwrap().file, "b.zip");
    assert!(gamebanana_file(&files, 33).is_none());
}

#[test]
fn a_trashed_mod_is_refused() {
    let server = serve(json!({ "_bIsTrashed": true, "_aFiles": [file(11, "a.zip", MD5_A)] }));
    let client = support::client(&server);
    let problem = gamebanana_files(&client, 4242).unwrap_err();
    assert!(problem.is_not_found());
    assert_eq!(
        problem.to_string(),
        "GameBanana mod 4242 is trashed or withheld"
    );
}

#[test]
fn a_withheld_mod_is_refused() {
    let server = serve(json!({ "_bIsWithheld": 1, "_aFiles": [file(11, "a.zip", MD5_A)] }));
    let client = support::client(&server);
    let problem = gamebanana_files(&client, 4242).unwrap_err();
    assert_eq!(
        problem.to_string(),
        "GameBanana mod 4242 is trashed or withheld"
    );
}

#[test]
fn an_empty_file_list_is_refused() {
    let server = serve(json!({ "_aFiles": [] }));
    let client = support::client(&server);
    let problem = gamebanana_files(&client, 4242).unwrap_err();
    assert!(problem.is_not_found());
    assert_eq!(
        problem.to_string(),
        "GameBanana mod 4242 publishes no files"
    );
}

#[test]
fn a_payload_that_is_not_an_object_is_unreachable() {
    let server = serve(json!([1, 2, 3]));
    let client = support::client(&server);
    let problem = gamebanana_files(&client, 4242).unwrap_err();
    assert!(!problem.is_not_found());
    assert!(
        problem.to_string().ends_with("unexpected response"),
        "{problem}"
    );
}

#[test]
fn a_lone_file_pins_itself_and_derives_the_id() {
    let server = serve(json!({ "_aFiles": [file(11, "Cool Mod v2.zip", MD5_A)] }));
    let client = support::client(&server);
    let pinned = pin_gamebanana(&client, 4242, None, None, &Map::new()).unwrap();
    match pinned {
        GbPin::Pinned { entry, note } => {
            assert_eq!(entry["id"], json!("cool-mod-v2"));
            assert_eq!(entry["source"], json!("gamebanana"));
            assert_eq!(entry["mod"], json!(4242));
            assert_eq!(entry["file"], json!(11));
            assert_eq!(entry["md5"], json!(MD5_A));
            assert_eq!(
                note,
                "gamebanana 4242 -> file 11 (Cool Mod v2.zip), md5 from the v11 API"
            );
        }
        GbPin::Choose { .. } => panic!("one file needs no choice"),
    }
}

#[test]
fn several_files_ask_the_caller_to_choose() {
    let server = serve(json!({ "_aFiles": [file(11, "a.zip", MD5_A), file(22, "b.zip", MD5_B)] }));
    let client = support::client(&server);
    match pin_gamebanana(&client, 4242, None, None, &Map::new()).unwrap() {
        GbPin::Choose { mod_id, files } => {
            assert_eq!(mod_id, 4242);
            assert_eq!(files.len(), 2);
        }
        GbPin::Pinned { .. } => panic!("two files cannot be picked for the caller"),
    }
}

#[test]
fn a_named_file_pins_and_keeps_options() {
    let server = serve(json!({ "_aFiles": [file(11, "a.zip", MD5_A), file(22, "b.zip", MD5_B)] }));
    let client = support::client(&server);
    let mut options = Map::new();
    options.insert("hard".into(), json!(true));
    match pin_gamebanana(&client, 4242, Some(22), Some("chosen"), &options).unwrap() {
        GbPin::Pinned { entry, .. } => {
            assert_eq!(entry["id"], json!("chosen"));
            assert_eq!(entry["file"], json!(22));
            assert_eq!(entry["md5"], json!(MD5_B));
            assert_eq!(entry["options"]["hard"], json!(true));
        }
        GbPin::Choose { .. } => panic!("an explicit file is not a choice"),
    }
}

#[test]
fn a_missing_file_id_lists_what_is_published() {
    let server = serve(json!({ "_aFiles": [file(11, "a.zip", MD5_A), file(22, "b.zip", MD5_B)] }));
    let client = support::client(&server);
    let problem = pin_gamebanana(&client, 4242, Some(99), None, &Map::new()).unwrap_err();
    assert!(problem.is_not_found());
    assert_eq!(
        problem.to_string(),
        "GameBanana mod 4242 has no file 99; it publishes 11 (a.zip), 22 (b.zip)"
    );
}

#[test]
fn a_file_with_no_md5_cannot_be_pinned() {
    let server = serve(json!({ "_aFiles": [file(11, "a.zip", "")] }));
    let client = support::client(&server);
    let problem = pin_gamebanana(&client, 4242, None, None, &Map::new()).unwrap_err();
    assert!(problem.is_not_found());
    assert_eq!(
        problem.to_string(),
        "GameBanana file 11 publishes no md5; a cart cannot pin it"
    );
}

#[test]
fn an_unnameable_file_takes_cartkits_generic_id() {
    let server = serve(json!({ "_aFiles": [file(11, "!!!.zip", MD5_A)] }));
    let client = support::client(&server);
    match pin_gamebanana(&client, 4242, None, None, &Map::new()).unwrap() {
        GbPin::Pinned { entry, .. } => assert_eq!(entry["id"], json!("mod")),
        GbPin::Choose { .. } => panic!("one file needs no choice"),
    }
}
