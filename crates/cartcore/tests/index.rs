use cartcore::index::{
    base_games_in, cache_fresh, can_install, categories_in, compat_issues, display_version,
    download_stats, filter, install_url, join_url, label_for, matches, parse_feed, release_dates,
    release_for, resolve_source, CompatContext, Entry, FilterOpts, Index, InstallKind, ModEntry,
    CACHE_TTL, CACHE_VERSION, SCHEMA_VERSION,
};
use serde_json::json;
use std::collections::HashMap;

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/index/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {}", path, err))
}

fn feed() -> Index {
    parse_feed(&fixture("feed.json")).expect("fixture feed parses")
}

fn mod_entry(update_check: &str) -> ModEntry {
    ModEntry {
        id: "fastrun".to_string(),
        title: "Fast Run".to_string(),
        version: Some("1.0.0".to_string()),
        update_check: update_check.to_string(),
        ..ModEntry::default()
    }
}

#[test]
fn constants_match_the_reference() {
    assert_eq!(CACHE_TTL, 24 * 60 * 60);
    assert_eq!(SCHEMA_VERSION, 1);
    assert_eq!(CACHE_VERSION, 3);
    assert!(cache_fresh(1_000, 3, 1_000 + CACHE_TTL - 1, CACHE_TTL));
    assert!(!cache_fresh(1_000, 3, 1_000 + CACHE_TTL, CACHE_TTL));
    assert!(!cache_fresh(1_000, 2, 1_000, CACHE_TTL));
}

#[test]
fn resolves_all_four_source_shapes() {
    let slug = resolve_source("kanto/modindex").expect("owner/repo");
    assert_eq!(
        slug.feed,
        "https://kanto.github.io/modindex/data/index.json"
    );
    assert_eq!(slug.base, "https://kanto.github.io/modindex/");
    assert_eq!(
        slug.fallback.as_deref(),
        Some("https://raw.githubusercontent.com/kanto/modindex/main/site/data/index.json")
    );
    assert_eq!(slug.label, "kanto/modindex");

    for url in [
        "https://github.com/kanto/modindex",
        "https://github.com/kanto/modindex.git",
        "http://github.com/kanto/modindex/tree/main/site",
        "  https://github.com/kanto/modindex  ",
    ] {
        assert_eq!(resolve_source(url).expect(url), slug, "{}", url);
    }

    let pages = resolve_source("https://kanto.github.io/modindex/").expect("pages root");
    assert_eq!(pages.feed, slug.feed);
    assert_eq!(pages.base, slug.base);
    assert_eq!(pages.fallback, None);
    assert_eq!(pages.label, "kanto/modindex");
    // a Pages root without its trailing slash grows one
    assert_eq!(
        resolve_source("https://kanto.github.io/modindex").expect("no slash"),
        pages
    );

    let direct =
        resolve_source("https://kanto.github.io/modindex/data/index.json").expect("feed url");
    assert_eq!(direct.feed, slug.feed);
    assert_eq!(direct.base, slug.base);
    assert_eq!(direct.fallback, None);
    assert_eq!(direct.label, "kanto/modindex");

    let other = resolve_source("https://mods.example.test/feeds/all.json").expect("other json");
    assert_eq!(other.feed, "https://mods.example.test/feeds/all.json");
    assert_eq!(other.base, "https://mods.example.test/feeds/");
    assert_eq!(other.label, "mods.example.test/feeds");

    let plain = resolve_source("https://mods.example.test/feeds").expect("plain root");
    assert_eq!(
        plain.feed,
        "https://mods.example.test/feeds/data/index.json"
    );
    assert_eq!(plain.base, "https://mods.example.test/feeds/");
}

#[test]
fn rejects_malformed_sources() {
    for input in ["", "   "] {
        assert_eq!(
            resolve_source(input).err().as_deref(),
            Some("missing index URL")
        );
    }
    for input in [
        "not a url",
        "kanto/modindex/extra",
        "ftp://example.test/data/index.json",
        "example.test",
        "kanto /modindex",
    ] {
        assert_eq!(
            resolve_source(input).err().as_deref(),
            Some("index must be an http(s) URL or owner/repo"),
            "{}",
            input
        );
    }
}

#[test]
fn labels_hosts_and_pages() {
    assert_eq!(
        label_for("https://kanto.github.io/modindex/"),
        "kanto/modindex"
    );
    assert_eq!(
        label_for("https://a.b.github.io/modindex/x"),
        "a.b/modindex"
    );
    assert_eq!(
        label_for("https://mods.example.test/feeds/"),
        "mods.example.test/feeds"
    );
    assert_eq!(label_for("https://mods.example.test/"), "mods.example.test");
    assert_eq!(label_for("mods.example.test"), "mods.example.test");
    assert_eq!(label_for(""), "");
}

#[test]
fn joins_relative_and_absolute_urls() {
    let base = "https://kanto.github.io/modindex/";
    assert_eq!(
        join_url(base, "thumbs/a.png").as_deref(),
        Some("https://kanto.github.io/modindex/thumbs/a.png")
    );
    assert_eq!(
        join_url(base, "/thumbs/a.png").as_deref(),
        Some("https://kanto.github.io/modindex/thumbs/a.png")
    );
    assert_eq!(
        join_url("https://kanto.github.io/modindex", "thumbs/a.png").as_deref(),
        Some("https://kanto.github.io/modindex/thumbs/a.png")
    );
    assert_eq!(
        join_url(base, "https://cdn.example.test/a.png").as_deref(),
        Some("https://cdn.example.test/a.png")
    );
    assert_eq!(join_url(base, ""), None);
    assert_eq!(join_url("", "thumbs/a.png"), None);
}

#[test]
fn schema_version_is_a_hard_gate() {
    let missing = json!({ "mods": [] }).to_string();
    assert_eq!(
        parse_feed(&missing).err().as_deref(),
        Some("index.json has no schema_version")
    );
    let not_a_number = json!({ "schema_version": "one", "mods": [] }).to_string();
    assert_eq!(
        parse_feed(&not_a_number).err().as_deref(),
        Some("index.json has no schema_version")
    );
    for (version, want) in [
        (
            json!(0),
            "index schema 0 is not supported (this build reads 1)",
        ),
        (
            json!(2),
            "index schema 2 is not supported (this build reads 1)",
        ),
    ] {
        let text = json!({ "schema_version": version, "mods": [] }).to_string();
        assert_eq!(parse_feed(&text).err().as_deref(), Some(want));
    }
    // a numeric string is still a number, as tonumber reads it
    let stringy = json!({ "schema_version": "1", "mods": [] }).to_string();
    assert_eq!(parse_feed(&stringy).expect("stringy").schema_version, 1);

    let no_mods = json!({ "schema_version": 1 }).to_string();
    assert_eq!(
        parse_feed(&no_mods).err().as_deref(),
        Some("index.json has no mods array")
    );
    assert_eq!(
        parse_feed("[1, 2, 3]").err().as_deref(),
        Some("index.json is not an object")
    );
    assert!(parse_feed("<html>404</html>")
        .err()
        .unwrap()
        .starts_with("could not read the index: "));
    assert!(parse_feed("").err().is_some());
}

#[test]
fn parses_a_feed_with_carts() {
    let index = feed();
    assert_eq!(index.schema_version, 1);
    assert_eq!(index.generated_at.as_deref(), Some("2026-02-11T04:15:09Z"));
    assert_eq!(index.base_games, ["red", "blue", "unused"]);

    let ids: Vec<&str> = index.mods.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        ["fastrun", "nightmode"],
        "rows without an id are dropped"
    );

    let fastrun = &index.mods[0];
    assert_eq!(fastrun.title, "Fast Run");
    assert_eq!(fastrun.author.as_deref(), Some("kanto"));
    assert_eq!(fastrun.api, Some(2.0));
    assert_eq!(fastrun.permissions, ["filesystem"]);
    assert_eq!(fastrun.update_check, "ok");
    let latest = fastrun.latest.as_ref().expect("latest");
    assert!(!latest.prerelease);
    assert_eq!(latest.tag.as_deref(), Some("v1.4.0"));
    let zip = latest.zip.as_ref().expect("zip");
    assert_eq!(zip.url, "https://example.test/fastrun-1.4.0.zip");
    assert_eq!(zip.size, Some(40211.0));

    let nightmode = &index.mods[1];
    assert_eq!(nightmode.summary, "Palette shifts after sunset.");
    assert!(nightmode.affects_link && nightmode.experimental);
    assert_eq!(nightmode.latest, None);
    // a bare number is a total-only download count
    let stats = download_stats(nightmode).expect("downloads");
    assert_eq!((stats.total, stats.recent), (Some(412.0), None));

    let cart_ids: Vec<&str> = index.carts.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(cart_ids, ["emberred", "bluetide"]);

    let ember = &index.carts[0];
    assert_eq!(ember.kind, "cart");
    assert_eq!(ember.seal, "sealed");
    assert_eq!(ember.speeds, [1.0, 2.0, 4.0]);
    assert!(!ember.automatic_version_check);
    assert_eq!(ember.load_order, ["fastrun", "skinpack"]);
    assert_eq!(
        ember.mods.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        ["fastrun", "skinpack"],
        "pins from unknown sources and pins without an id are dropped"
    );
    assert_eq!(ember.mods[0].enabled, Some(false));
    assert_eq!(ember.mods[1].mod_id, Some(55123.0));
    assert_eq!(ember.mods[1].file, Some(99001.0));
    assert!(ember.mods[1].options.is_some());
    assert_eq!(ember.mods[1].enabled, None);

    let blue = &index.carts[1];
    assert_eq!(blue.summary, "");
    assert!(blue.automatic_version_check, "absent means on");
    assert_eq!(blue.update_check, "pending");
}

#[test]
fn drops_incomplete_and_unpinned_carts() {
    let index = feed();
    for dropped in ["missingseal", "nopins", "nomods"] {
        assert!(
            !index.carts.iter().any(|c| c.id == dropped),
            "{} should be dropped",
            dropped
        );
    }
}

#[test]
fn parses_a_feed_without_carts() {
    let index = parse_feed(&fixture("feed_nocarts.json")).expect("no-cart feed");
    assert_eq!(index.mods.len(), 1);
    assert!(index.carts.is_empty());
    assert!(index.base_games.is_empty());
    // an absent title falls back to the id, an absent summary to ""
    assert_eq!(index.mods[0].title, "corelib");
    assert_eq!(index.mods[0].version, None);
}

#[test]
fn install_url_follows_release_then_download() {
    let index = feed();
    let fastrun = &index.mods[0];
    assert_eq!(
        install_url(fastrun).expect("release"),
        (
            "https://example.test/fastrun-1.4.0.zip".to_string(),
            InstallKind::Release
        )
    );
    assert!(can_install(fastrun));
    assert_eq!(display_version(fastrun), "1.4.0");
    assert_eq!(
        release_for(fastrun).expect("release"),
        *fastrun.latest.as_ref().unwrap()
    );

    let nightmode = &index.mods[1];
    assert_eq!(
        install_url(nightmode).expect("download"),
        (
            "https://example.test/nightmode.zip".to_string(),
            InstallKind::Download
        )
    );
    let synthesised = release_for(nightmode).expect("synthesised");
    assert_eq!(synthesised.version.as_deref(), Some("0.9.1"));
    let zip = synthesised.zip.as_ref().expect("zip");
    assert_eq!(zip.url, "https://example.test/nightmode.zip");
    assert_eq!(zip.name.as_deref(), Some("nightmode.zip"));

    // "ok" with no zip behind it still falls through
    let mut ok_no_zip = mod_entry("ok");
    ok_no_zip.latest = Some(Default::default());
    assert_eq!(
        install_url(&ok_no_zip).err().as_deref(),
        Some("nothing installable listed")
    );
    assert_eq!(
        display_version(&ok_no_zip),
        "1.0.0",
        "no release version to show"
    );

    for (update_check, want) in [
        ("off", "the author does not publish installable releases"),
        ("no installable release", "no release with a .zip asset yet"),
        ("error: rate limited", "error: rate limited"),
        ("pending", "nothing installable listed"),
        ("", "nothing installable listed"),
    ] {
        let entry = mod_entry(update_check);
        assert_eq!(
            install_url(&entry).err().as_deref(),
            Some(want),
            "{}",
            update_check
        );
        assert!(!can_install(&entry));
        assert_eq!(release_for(&entry).err().as_deref(), Some(want));
    }

    // a downloadURL outranks every failing update_check
    let mut with_url = mod_entry("off");
    with_url.download_url = Some("https://example.test/x.zip".to_string());
    assert_eq!(
        install_url(&with_url).expect("download").1,
        InstallKind::Download
    );
    with_url.download_url = Some(String::new());
    assert!(!can_install(&with_url), "an empty downloadURL is no URL");

    let mut unversioned = mod_entry("pending");
    unversioned.version = None;
    assert_eq!(display_version(&unversioned), "?");
}

#[test]
fn release_dates_prefer_the_declared_days() {
    let index = feed();
    let dates = release_dates(&index.mods[0]).expect("dates");
    assert_eq!(dates.first.as_deref(), Some("2025-06-02"));
    assert_eq!(dates.latest.as_deref(), Some("2026-01-30"));

    let mut only_latest = mod_entry("ok");
    only_latest.latest = Some(cartcore::index::Latest {
        published_at: Some("2026-02-01T09:00:00Z".to_string()),
        ..Default::default()
    });
    let dates = release_dates(&only_latest).expect("dates");
    assert_eq!(dates.first, None);
    assert_eq!(dates.latest.as_deref(), Some("2026-02-01"));

    let mut bad_day = mod_entry("ok");
    bad_day.first_release = Some("last tuesday".to_string());
    assert!(release_dates(&bad_day).is_none());
    assert!(release_dates(&mod_entry("ok")).is_none());
    assert!(download_stats(&mod_entry("ok")).is_none());
}

#[test]
fn compat_issues_cover_every_branch() {
    let ctx = CompatContext {
        mod_api: Some(1.0),
        engine_version: Some("1.0.0".to_string()),
        installed: HashMap::from([("corelib".to_string(), "1.2.0".to_string())]),
    };
    let entry = ModEntry {
        id: "kitchen".to_string(),
        api: Some(2.0),
        game_version: Some(">=2.0.0".to_string()),
        profile: Some("engine".to_string()),
        affects_link: true,
        experimental: true,
        permissions: vec!["filesystem".to_string(), "network".to_string()],
        dependencies: Some(json!(["missingdep@^1.0.0", "corelib"])),
        conflicts: Some(json!(["corelib"])),
        ..ModEntry::default()
    };
    let texts: Vec<String> = compat_issues(&entry, &ctx)
        .into_iter()
        .map(|issue| {
            assert_eq!(issue.level, "warn");
            issue.text
        })
        .collect();
    assert_eq!(
        texts,
        [
            "Needs mod API 2; this build provides 1",
            "Needs engine >=2.0.0 (have 1.0.0)",
            "Profile 'engine' changes engine behaviour beyond content",
            "Changes link play; both sides need the same mods",
            "Marked experimental by its author",
            "Requests permission: filesystem",
            "Requests permission: network",
            "Needs missingdep ^1.0.0 (not installed)",
            "Conflicts with installed corelib",
        ]
    );

    // the quiet side of every branch
    let calm = ModEntry {
        id: "calm".to_string(),
        api: Some(1.0),
        game_version: Some(">=1.0.0 <2.0.0".to_string()),
        profile: Some("content".to_string()),
        dependencies: Some(json!(["corelib@^1.0.0"])),
        conflicts: Some(json!(["missingdep"])),
        ..ModEntry::default()
    };
    assert!(compat_issues(&calm, &ctx).is_empty());

    // an empty ctx installs nothing, so only the dependency is missing
    let no_ctx = CompatContext::default();
    assert_eq!(
        compat_issues(&calm, &no_ctx)
            .into_iter()
            .map(|issue| issue.text)
            .collect::<Vec<_>>(),
        ["Needs corelib ^1.0.0 (not installed)"]
    );

    // an unparsable engine version is a miss, so it warns
    let odd_engine = CompatContext {
        engine_version: Some("nightly".to_string()),
        ..CompatContext::default()
    };
    assert_eq!(
        compat_issues(&calm, &odd_engine)
            .into_iter()
            .map(|issue| issue.text)
            .collect::<Vec<_>>(),
        [
            "Needs engine >=1.0.0 <2.0.0 (have nightly)",
            "Needs corelib ^1.0.0 (not installed)",
        ]
    );
}

#[test]
fn dependency_specs_speak_the_manifest_vocabulary() {
    let ctx = CompatContext::default();
    let cases: &[(serde_json::Value, &[&str])] = &[
        (json!(["plain"]), &["Needs plain (not installed)"]),
        (
            json!(["pinned@^1.2"]),
            &["Needs pinned ^1.2 (not installed)"],
        ),
        (
            json!(["forked@>=1.0.0#kanto/forked"]),
            &["Needs forked >=1.0.0 (not installed)"],
        ),
        (
            json!(["hashonly#kanto/repo"]),
            &["Needs hashonly (not installed)"],
        ),
        (
            json!([{ "id": "tabled", "range": ">=2" }]),
            &["Needs tabled >=2 (not installed)"],
        ),
        (
            json!([{ "id": "versioned", "version": "1.0.0", "github": "kanto/versioned" }]),
            &["Needs versioned 1.0.0 (not installed)"],
        ),
        (json!([{ "id": "bare" }]), &["Needs bare (not installed)"]),
        (json!([{ "range": "^1" }]), &[]),
        (
            json!({ "mapped": "^3.1" }),
            &["Needs mapped ^3.1 (not installed)"],
        ),
        (json!({ "loose": true }), &["Needs loose (not installed)"]),
        (json!("just a string"), &[]),
        (json!(42), &[]),
        (json!([7, null, true]), &[]),
    ];
    for (spec, want) in cases {
        let entry = ModEntry {
            dependencies: Some(spec.clone()),
            ..ModEntry::default()
        };
        let texts: Vec<String> = compat_issues(&entry, &ctx)
            .into_iter()
            .map(|issue| issue.text)
            .collect();
        assert_eq!(texts, *want, "{}", spec);
    }
}

#[test]
fn matches_and_filter_narrow_the_listing() {
    let index = feed();
    let fastrun = &index.mods[0];
    assert!(matches(fastrun, ""));
    assert!(matches(fastrun, "   "));
    assert!(matches(fastrun, "fast"));
    assert!(
        matches(fastrun, "FAST kanto"),
        "terms AND, case-insensitively"
    );
    assert!(matches(fastrun, "bike"), "the summary is searched");
    assert!(!matches(fastrun, "fast johto"));

    let all = filter(&index.mods, &FilterOpts::default());
    assert_eq!(all.len(), 2);

    let by_query = filter(
        &index.mods,
        &FilterOpts {
            query: Some("night".to_string()),
            ..FilterOpts::default()
        },
    );
    assert_eq!(
        by_query.iter().map(|m| m.id()).collect::<Vec<_>>(),
        ["nightmode"]
    );

    let by_category = filter(
        &index.mods,
        &FilterOpts {
            category: Some("quality of life".to_string()),
            ..FilterOpts::default()
        },
    );
    assert_eq!(
        by_category.iter().map(|m| m.id()).collect::<Vec<_>>(),
        ["fastrun"]
    );

    let by_tag = filter(
        &index.mods,
        &FilterOpts {
            tag: Some("PALETTE".to_string()),
            ..FilterOpts::default()
        },
    );
    assert_eq!(
        by_tag.iter().map(|m| m.id()).collect::<Vec<_>>(),
        ["nightmode"]
    );

    let by_base = filter(
        &index.carts,
        &FilterOpts {
            base: Some("Red".to_string()),
            ..FilterOpts::default()
        },
    );
    assert_eq!(
        by_base.iter().map(|c| c.id()).collect::<Vec<_>>(),
        ["emberred"]
    );

    let nothing = filter(
        &index.mods,
        &FilterOpts {
            query: Some("fast".to_string()),
            category: Some("overhaul".to_string()),
            ..FilterOpts::default()
        },
    );
    assert!(nothing.is_empty());
}

#[test]
fn category_and_base_lists_follow_the_feed_order() {
    let mut index = feed();
    assert_eq!(categories_in(&index), ["Quality of life", "Overhaul"]);
    assert_eq!(base_games_in(&index), ["red", "blue"]);

    index.mods[1].categories.push("Uncounted".to_string());
    assert_eq!(
        categories_in(&index),
        ["Quality of life", "Overhaul", "Uncounted"],
        "a category the header forgot is appended"
    );
    index.carts[1].base = "gold".to_string();
    assert_eq!(base_games_in(&index), ["red", "gold"]);
}
