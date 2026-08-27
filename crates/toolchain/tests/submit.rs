mod common;

use serde_json::{json, Value};
use tempdir::TempDir;
use toolchain::fake::FakeRunner;
use toolchain::readiness::IndexHints;
use toolchain::runner::{CancelToken, Output};
use toolchain::submit::{
    data_file_in, discover, merge_entry, parse_issue_form, plan, submit_issue, submit_pull_request,
    SubmissionKind, INDEX_REPO,
};

const FORM: &str = r#"name: Add a cart
description: Submit a cart repository to the index
title: "Add cart: "
labels: [cart, submission]
body:
  - type: markdown
    attributes:
      value: Thanks for submitting.
  - type: input
    id: repo
    attributes:
      label: Cart repository (owner/name)
    validations:
      required: true
  - type: input
    id: version
    attributes:
      label: Version
    validations:
      required: true
  - type: textarea
    id: summary
    attributes:
      label: Summary
  - type: input
    id: license
    attributes:
      label: License
"#;

const CONTRIBUTING_PR: &str =
    "# Contributing\n\nOpen a pull request that edits `site/data/carts.json` with your row.\n";

fn cart() -> cartcore::cart::Cart {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path())
}

fn hints() -> IndexHints {
    IndexHints {
        license: Some("MIT".to_string()),
        tags: vec!["kanto".to_string(), "night".to_string()],
        ..IndexHints::default()
    }
}

fn issue_form_repo() -> FakeRunner {
    let fake = FakeRunner::new();
    fake.on(
        "gh",
        &["api", "repos/bryanthaboi/gen1recomp-mod-index/contents/.github/ISSUE_TEMPLATE"],
        Output::ok(
            json!([
                { "name": "config.yml", "path": ".github/ISSUE_TEMPLATE/config.yml", "type": "file" },
                { "name": "bug.md", "path": ".github/ISSUE_TEMPLATE/bug.md", "type": "file" },
                { "name": "add-cart.yml", "path": ".github/ISSUE_TEMPLATE/add-cart.yml", "type": "file" },
            ])
            .to_string(),
        ),
    );
    fake.on(
        "gh",
        &[
            "api",
            "repos/bryanthaboi/gen1recomp-mod-index/contents/.github/ISSUE_TEMPLATE/add-cart.yml",
        ],
        Output::ok(FORM),
    );
    fake.on(
        "gh",
        &[
            "api",
            "repos/bryanthaboi/gen1recomp-mod-index/contents/CONTRIBUTING.md",
        ],
        Output::ok("# Contributing\n\nUse the issue form.\n"),
    );
    fake
}

fn pull_request_repo() -> FakeRunner {
    let fake = FakeRunner::new();
    fake.on(
        "gh",
        &[
            "api",
            "repos/bryanthaboi/gen1recomp-mod-index/contents/.github/ISSUE_TEMPLATE",
        ],
        Output::fail(1, "gh: Not Found (HTTP 404)"),
    );
    fake.on(
        "gh",
        &[
            "api",
            "repos/bryanthaboi/gen1recomp-mod-index/contents/CONTRIBUTING.md",
        ],
        Output::ok(CONTRIBUTING_PR),
    );
    fake
}

#[test]
fn an_issue_form_is_read_field_by_field() {
    let template = parse_issue_form(".github/ISSUE_TEMPLATE/add-cart.yml", "add-cart.yml", FORM);
    assert!(template.form);
    assert_eq!(template.name, "Add a cart");
    assert_eq!(template.title.as_deref(), Some("Add cart: "));
    assert_eq!(template.labels, vec!["cart", "submission"]);
    let ids: Vec<&str> = template
        .fields
        .iter()
        .map(|field| field.id.as_str())
        .collect();
    assert_eq!(ids, vec!["repo", "version", "summary", "license"]);
    assert_eq!(template.fields[0].label, "Cart repository (owner/name)");
    assert!(template.fields[0].required);
    assert!(!template.fields[2].required);
    assert_eq!(template.fields[2].kind, "textarea");
}

#[test]
fn discovery_reads_the_index_repo_through_gh_api() {
    let fake = issue_form_repo();
    let found = discover(&fake, &CancelToken::new(), INDEX_REPO).expect("discover");
    assert_eq!(found.repo, INDEX_REPO);
    assert_eq!(found.templates.len(), 1);
    assert!(found.contributing.is_some());
    assert_eq!(
        fake.argv_log()[0],
        vec![
            "gh",
            "api",
            "repos/bryanthaboi/gen1recomp-mod-index/contents/.github/ISSUE_TEMPLATE",
            "-H",
            "Accept: application/vnd.github+json"
        ]
    );
    assert_eq!(
        fake.argv_log()[1],
        vec![
            "gh",
            "api",
            "repos/bryanthaboi/gen1recomp-mod-index/contents/.github/ISSUE_TEMPLATE/add-cart.yml",
            "-H",
            "Accept: application/vnd.github.raw"
        ]
    );
}

#[test]
fn an_issue_form_repo_produces_a_prefilled_issue_plan() {
    let fake = issue_form_repo();
    let found = discover(&fake, &CancelToken::new(), INDEX_REPO).expect("discover");
    let plan = plan(&found, &cart(), "bryanthaboi/night-run", &hints());

    assert_eq!(plan.kind, SubmissionKind::IssueForm);
    assert_eq!(plan.repo, INDEX_REPO);
    assert_eq!(plan.title, "Add cart: Night Run 1.0.0");
    assert_eq!(plan.labels, vec!["cart", "submission"]);
    assert_eq!(plan.template.as_deref(), Some("add-cart.yml"));
    assert!(plan.data_file.is_none());

    let value = |id: &str| {
        plan.fields
            .iter()
            .find(|field| field.id == id)
            .unwrap()
            .value
            .clone()
    };
    assert_eq!(value("repo"), "bryanthaboi/night-run");
    assert_eq!(value("version"), "1.0.0");
    assert_eq!(value("summary"), "A short run through Kanto at night.");
    assert_eq!(value("license"), "MIT");
    assert!(plan.body.contains("### Cart repository (owner/name)"));
    assert!(plan.body.contains("bryanthaboi/night-run"));
    assert!(!plan.guidance.is_empty());
}

#[test]
fn the_issue_is_created_from_the_reviewed_plan() {
    let fake = issue_form_repo();
    let found = discover(&fake, &CancelToken::new(), INDEX_REPO).expect("discover");
    let mut plan = plan(&found, &cart(), "bryanthaboi/night-run", &hints());
    plan.body = "### Cart repository (owner/name)\n\nbryanthaboi/night-run".to_string();

    let post = FakeRunner::new();
    post.on(
        "gh",
        &["issue", "create"],
        Output::ok("https://github.com/bryanthaboi/gen1recomp-mod-index/issues/17\n"),
    );
    let submission = submit_issue(&post, &CancelToken::new(), &plan).expect("submit");
    assert_eq!(
        submission.url.as_deref(),
        Some("https://github.com/bryanthaboi/gen1recomp-mod-index/issues/17")
    );
    assert_eq!(
        post.argv_log(),
        vec![vec![
            "gh",
            "issue",
            "create",
            "--repo",
            INDEX_REPO,
            "--title",
            "Add cart: Night Run 1.0.0",
            "--body",
            "### Cart repository (owner/name)\n\nbryanthaboi/night-run",
            "--label",
            "cart",
            "--label",
            "submission"
        ]]
    );
}

#[test]
fn a_pull_request_repo_produces_a_pull_request_plan() {
    let fake = pull_request_repo();
    let found = discover(&fake, &CancelToken::new(), INDEX_REPO).expect("discover");
    assert_eq!(found.data_file.as_deref(), Some("site/data/carts.json"));
    assert!(found.templates.is_empty());

    let plan = plan(&found, &cart(), "bryanthaboi/night-run", &hints());
    assert_eq!(plan.kind, SubmissionKind::PullRequest);
    assert_eq!(plan.data_file.as_deref(), Some("site/data/carts.json"));
    assert_eq!(plan.branch.as_deref(), Some("add-cart-night-run"));
    let entry = plan.entry.clone().expect("entry");
    assert_eq!(entry.get("id").unwrap(), "night-run");
    assert_eq!(entry.get("repo").unwrap(), "bryanthaboi/night-run");
    assert_eq!(entry.get("license").unwrap(), "MIT");
    assert_eq!(entry.get("tags").unwrap(), &json!(["kanto", "night"]));
    assert_eq!(entry.get("mods").unwrap().as_array().unwrap().len(), 1);
}

#[test]
fn the_pull_request_path_forks_branches_edits_and_opens_the_pr() {
    let discovery_runner = pull_request_repo();
    let found = discover(&discovery_runner, &CancelToken::new(), INDEX_REPO).expect("discover");
    let plan = plan(&found, &cart(), "bryanthaboi/night-run", &hints());

    let work = TempDir::new("fork").expect("temp");
    let clone = work.path().join("gen1recomp-mod-index");
    std::fs::create_dir_all(clone.join("site/data")).expect("dirs");
    std::fs::write(
        clone.join("site/data/carts.json"),
        json!([{ "id": "other-cart", "title": "Other" }]).to_string(),
    )
    .expect("seed");

    let fake = FakeRunner::new();
    fake.on("gh", &["api", "user"], Output::ok("cartauthor\n"));
    fake.on("gh", &["repo", "fork"], Output::ok(""));
    fake.on("git", &["checkout"], Output::ok(""));
    fake.on("git", &["add"], Output::ok(""));
    fake.on("git", &["commit"], Output::ok(""));
    fake.on("git", &["push"], Output::ok(""));
    fake.on(
        "gh",
        &["pr", "create"],
        Output::ok("https://github.com/bryanthaboi/gen1recomp-mod-index/pull/9\n"),
    );

    let submission =
        submit_pull_request(&fake, &CancelToken::new(), &plan, work.path()).expect("submit");
    assert_eq!(
        submission.url.as_deref(),
        Some("https://github.com/bryanthaboi/gen1recomp-mod-index/pull/9")
    );

    assert_eq!(
        fake.argv_log(),
        vec![
            vec!["gh", "api", "user", "--jq", ".login"],
            vec![
                "gh",
                "repo",
                "fork",
                INDEX_REPO,
                "--clone",
                "--remote=false"
            ],
            vec!["git", "checkout", "-b", "add-cart-night-run"],
            vec!["git", "add", "--", "site/data/carts.json"],
            vec!["git", "commit", "-m", "Add cart: Night Run 1.0.0"],
            vec!["git", "push", "-u", "origin", "add-cart-night-run"],
            vec![
                "gh",
                "pr",
                "create",
                "--repo",
                INDEX_REPO,
                "--title",
                "Add cart: Night Run 1.0.0",
                "--body",
                plan.body.as_str(),
                "--head",
                "cartauthor:add-cart-night-run"
            ],
        ]
    );

    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(clone.join("site/data/carts.json")).unwrap())
            .expect("json");
    let rows = written.as_array().expect("array");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].get("id").unwrap(), "night-run");
}

#[test]
fn a_row_with_the_same_id_is_replaced_not_duplicated() {
    let dir = TempDir::new("data").expect("temp");
    let path = dir.path().join("carts.json");
    std::fs::write(
        &path,
        json!({ "schema_version": 1, "carts": [{ "id": "night-run", "version": "0.9.0" }] })
            .to_string(),
    )
    .expect("seed");
    merge_entry(&path, &json!({ "id": "night-run", "version": "1.0.0" })).expect("merge");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).expect("json");
    let rows = doc.get("carts").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("version").unwrap(), "1.0.0");
    assert_eq!(doc.get("schema_version").unwrap(), 1);
}

#[test]
fn a_missing_data_file_starts_a_list() {
    let dir = TempDir::new("data").expect("temp");
    let path = dir.path().join("nested/carts.json");
    merge_entry(&path, &json!({ "id": "night-run" })).expect("merge");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).expect("json");
    assert_eq!(doc.as_array().unwrap().len(), 1);
}

#[test]
fn contributing_is_scanned_for_the_data_file_it_names() {
    assert_eq!(
        data_file_in("Edit `site/data/carts.json` and open a PR.").as_deref(),
        Some("site/data/carts.json")
    );
    assert_eq!(
        data_file_in("See https://example.com/data/index.json for the feed.").as_deref(),
        None
    );
    assert_eq!(data_file_in("Just open an issue.").as_deref(), None);
}

#[test]
fn a_repo_with_neither_a_form_nor_a_data_file_falls_back_to_a_plain_issue() {
    let fake = FakeRunner::new();
    fake.on("gh", &["api"], Output::fail(1, "gh: Not Found (HTTP 404)"));
    let found = discover(&fake, &CancelToken::new(), INDEX_REPO).expect("discover");
    let plan = plan(&found, &cart(), "bryanthaboi/night-run", &hints());
    assert_eq!(plan.kind, SubmissionKind::Issue);
    assert!(plan.body.contains("### Cart repository"));
    assert!(plan.guidance[0].contains("plain issue"));
}

#[test]
fn a_failed_submission_carries_the_cause_and_the_commands_it_ran() {
    let fake = FakeRunner::new();
    fake.on(
        "gh",
        &["issue", "create"],
        Output::fail(1, "HTTP 401: Bad credentials"),
    );
    let discovery_runner = issue_form_repo();
    let found = discover(&discovery_runner, &CancelToken::new(), INDEX_REPO).expect("discover");
    let plan = plan(&found, &cart(), "bryanthaboi/night-run", &hints());
    let problem = submit_issue(&fake, &CancelToken::new(), &plan).expect_err("auth");
    assert_eq!(problem.cause, toolchain::publish::Cause::AuthExpired);
    assert_eq!(problem.step, "gh issue create");
    assert!(problem.stderr.contains("Bad credentials"));
}
