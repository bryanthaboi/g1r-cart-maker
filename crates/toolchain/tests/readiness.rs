mod common;

use serde_json::json;
use tempdir::TempDir;
use toolchain::fake::FakeRunner;
use toolchain::readiness::{
    check, evaluate, fetch_remote, IndexHints, Readiness, ReleaseFacts, RemoteFacts,
};
use toolchain::runner::{CancelToken, Output};

const SLUG: &str = "bryanthaboi/night-run";
const ASSET: &str = "night-run-1.0.0.g1rcart";

fn cart() -> cartcore::cart::Cart {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path())
}

fn listed() -> RemoteFacts {
    RemoteFacts {
        slug: SLUG.to_string(),
        visibility: Some("public".to_string()),
        is_private: Some(false),
        url: Some(format!("https://github.com/{}", SLUG)),
        license: Some("MIT".to_string()),
        release: Some(ReleaseFacts {
            tag: "v1.0.0".to_string(),
            url: None,
            assets: vec![ASSET.to_string(), "sha256sums.txt".to_string()],
        }),
        problems: Vec::new(),
    }
}

fn hints() -> IndexHints {
    IndexHints {
        thumbnail: Some("thumb.png".to_string()),
        description_url: Some("https://example.com/night-run".to_string()),
        license: Some("MIT".to_string()),
        tags: vec!["kanto".to_string()],
        automatic_version_check: Some(true),
        fixed_release_tag: None,
    }
}

fn item<'a>(readiness: &'a Readiness, id: &str) -> &'a toolchain::readiness::ReadinessItem {
    readiness
        .items
        .iter()
        .find(|item| item.id == id)
        .unwrap_or_else(|| panic!("no item {}", id))
}

#[test]
fn a_fully_prepared_cart_is_ready() {
    let readiness = evaluate(&cart(), Some(&listed()), &hints());
    assert!(readiness.ready);
    assert!(
        readiness.items.iter().all(|item| item.ok),
        "{:?}",
        readiness.items
    );
    assert!(readiness.unknown.is_empty());
    for id in [
        "repo_public",
        "cart_schema",
        "required_fields",
        "valid_pin",
        "release_tag",
        "bundle_asset",
        "sha256sums",
        "thumbnail",
        "description_url",
        "license",
        "summary",
        "tags",
        "version_check",
    ] {
        assert!(item(&readiness, id).ok, "{} should be met", id);
    }
    assert_eq!(readiness.items.len(), 13);
}

#[test]
fn the_six_gate_items_are_the_blocking_ones() {
    let readiness = evaluate(&cart(), Some(&listed()), &hints());
    let blocking: Vec<&str> = readiness
        .items
        .iter()
        .filter(|item| item.blocking)
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(
        blocking,
        vec![
            "repo_public",
            "cart_schema",
            "required_fields",
            "valid_pin",
            "release_tag",
            "bundle_asset"
        ]
    );
}

#[test]
fn a_private_repo_blocks_and_offers_the_fix_command() {
    let mut facts = listed();
    facts.is_private = Some(true);
    facts.visibility = Some("private".to_string());
    let readiness = evaluate(&cart(), Some(&facts), &hints());
    assert!(!readiness.ready);
    let public = item(&readiness, "repo_public");
    assert!(!public.ok);
    assert_eq!(
        public.fix.as_ref().unwrap().command.as_ref().unwrap(),
        &vec![
            "gh".to_string(),
            "repo".to_string(),
            "edit".to_string(),
            SLUG.to_string(),
            "--visibility".to_string(),
            "public".to_string()
        ]
    );
}

#[test]
fn a_repo_gh_could_not_read_is_unknown_rather_than_failed_silently() {
    let facts = RemoteFacts {
        slug: SLUG.to_string(),
        ..RemoteFacts::default()
    };
    let readiness = evaluate(&cart(), Some(&facts), &hints());
    assert_eq!(readiness.unknown, vec!["repo_public"]);
    assert!(!item(&readiness, "repo_public").ok);
    assert_eq!(
        item(&readiness, "repo_public").fix.as_ref().unwrap().id,
        "recheck"
    );
}

#[test]
fn a_wrong_schema_a_missing_field_and_a_placeholder_pin_all_block() {
    let mut cart = cart();
    cart.insert("schema".to_string(), json!(2));
    cart.remove("author");
    cart.insert(
        "mods".to_string(),
        json!([{ "id": "night-mode", "source": "github", "repo": "owner/night-mode" }]),
    );
    let readiness = evaluate(&cart, Some(&listed()), &hints());
    assert!(!item(&readiness, "cart_schema").ok);
    assert!(item(&readiness, "cart_schema").detail.contains("schema 2"));
    assert!(!item(&readiness, "required_fields").ok);
    assert!(item(&readiness, "required_fields")
        .detail
        .contains("author"));
    assert!(!item(&readiness, "valid_pin").ok);
    assert!(!readiness.ready);
}

#[test]
fn a_gamebanana_pin_counts_as_a_valid_pin() {
    let mut cart = cart();
    cart.insert(
        "mods".to_string(),
        json!([{
            "id": "night-mode",
            "source": "gamebanana",
            "mod": "546899",
            "file": "1234567",
            "md5": "0123456789abcdef0123456789abcdef",
        }]),
    );
    let readiness = evaluate(&cart, Some(&listed()), &hints());
    assert!(item(&readiness, "valid_pin").ok);
}

#[test]
fn a_release_tag_that_does_not_match_the_cart_blocks() {
    let mut facts = listed();
    facts.release = Some(ReleaseFacts {
        tag: "v0.9.0".to_string(),
        url: None,
        assets: vec![ASSET.to_string()],
    });
    let readiness = evaluate(&cart(), Some(&facts), &hints());
    let tag = item(&readiness, "release_tag");
    assert!(!tag.ok);
    assert!(tag.detail.contains("v0.9.0") && tag.detail.contains("v1.0.0"));
    assert_eq!(tag.fix.as_ref().unwrap().id, "publish_release");
}

#[test]
fn a_release_with_no_bundle_blocks_and_sha_sums_only_warns() {
    let mut facts = listed();
    facts.release = Some(ReleaseFacts {
        tag: "v1.0.0".to_string(),
        url: None,
        assets: vec!["night-run.zip".to_string()],
    });
    let readiness = evaluate(&cart(), Some(&facts), &hints());
    let bundle = item(&readiness, "bundle_asset");
    assert!(!bundle.ok && bundle.blocking);
    assert!(bundle.detail.contains("night-run.zip"));
    let sums = item(&readiness, "sha256sums");
    assert!(!sums.ok && !sums.blocking);
    assert!(!readiness.ready);
}

#[test]
fn a_missing_release_blocks_both_release_items() {
    let mut facts = listed();
    facts.release = None;
    let readiness = evaluate(&cart(), Some(&facts), &hints());
    assert!(!item(&readiness, "release_tag").ok);
    assert!(!item(&readiness, "bundle_asset").ok);
    assert!(item(&readiness, "bundle_asset")
        .detail
        .contains("no assets"));
}

#[test]
fn the_recommended_items_never_block() {
    let readiness = evaluate(&cart(), Some(&listed()), &IndexHints::default());
    for id in ["thumbnail", "description_url", "tags"] {
        let entry = item(&readiness, id);
        assert!(!entry.ok && !entry.blocking, "{}", id);
        assert_eq!(entry.fix.as_ref().unwrap().id, "edit_entry");
    }
    assert!(
        readiness.ready,
        "recommendations alone never keep a cart out"
    );
}

#[test]
fn a_repo_license_stands_in_for_one_the_cart_does_not_carry() {
    let readiness = evaluate(&cart(), Some(&listed()), &IndexHints::default());
    let license = item(&readiness, "license");
    assert!(license.ok);
    assert_eq!(license.detail, "MIT");
}

#[test]
fn a_summary_over_the_limit_is_flagged() {
    let mut cart = cart();
    cart.insert("summary".to_string(), json!("x".repeat(121)));
    let readiness = evaluate(&cart, Some(&listed()), &hints());
    let summary = item(&readiness, "summary");
    assert!(!summary.ok);
    assert!(summary.detail.contains("121 characters"));
}

#[test]
fn version_checking_off_without_a_fixed_tag_is_flagged() {
    let off = IndexHints {
        automatic_version_check: Some(false),
        ..hints()
    };
    let readiness = evaluate(&cart(), Some(&listed()), &off);
    assert!(!item(&readiness, "version_check").ok);

    let pinned = IndexHints {
        automatic_version_check: Some(false),
        fixed_release_tag: Some("v1.0.0".to_string()),
        ..hints()
    };
    let readiness = evaluate(&cart(), Some(&listed()), &pinned);
    assert!(item(&readiness, "version_check").ok);
    assert!(item(&readiness, "version_check").detail.contains("v1.0.0"));
}

#[test]
fn hints_fall_back_to_a_cart_that_carries_the_fields_itself() {
    let mut cart = cart();
    cart.insert("thumbnail".to_string(), json!("thumb.png"));
    cart.insert("tags".to_string(), json!(["kanto", "night"]));
    cart.insert("automatic_version_check".to_string(), json!(false));
    let hints = IndexHints::from_cart(&cart);
    assert_eq!(hints.thumbnail.as_deref(), Some("thumb.png"));
    assert_eq!(hints.tags.len(), 2);
    assert_eq!(hints.automatic_version_check, Some(false));
}

#[test]
fn the_remote_facts_come_from_two_gh_calls_with_exact_arrays() {
    let fake = FakeRunner::new();
    fake.on(
        "gh",
        &["repo", "view"],
        Output::ok(
            json!({
                "visibility": "PUBLIC",
                "isPrivate": false,
                "url": format!("https://github.com/{}", SLUG),
                "licenseInfo": { "spdxId": "MIT" },
            })
            .to_string(),
        ),
    );
    fake.on(
        "gh",
        &["release", "view"],
        Output::ok(common::release_view("v1.0.0", &[ASSET])),
    );
    let facts = fetch_remote(&fake, &CancelToken::new(), SLUG, "v1.0.0").expect("facts");
    assert_eq!(facts.visibility.as_deref(), Some("public"));
    assert_eq!(facts.license.as_deref(), Some("MIT"));
    assert_eq!(facts.release.unwrap().assets, vec![ASSET]);
    assert_eq!(
        fake.argv_log(),
        vec![
            vec![
                "gh",
                "repo",
                "view",
                SLUG,
                "--json",
                "visibility,isPrivate,url,licenseInfo"
            ],
            vec![
                "gh",
                "release",
                "view",
                "v1.0.0",
                "--repo",
                SLUG,
                "--json",
                "tagName,url,assets"
            ],
        ]
    );
}

#[test]
fn a_failed_gh_call_becomes_an_unknown_fact_not_a_hard_no() {
    let fake = FakeRunner::new();
    fake.on(
        "gh",
        &["repo", "view"],
        Output::fail(1, "HTTP 404: Not Found"),
    );
    fake.on(
        "gh",
        &["release", "view"],
        Output::fail(1, "release not found"),
    );
    let readiness = check(&fake, &CancelToken::new(), &cart(), SLUG, &hints()).expect("check");
    assert!(!readiness.ready);
    assert_eq!(readiness.unknown, vec!["repo_public"]);
}
