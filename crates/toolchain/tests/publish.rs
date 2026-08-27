mod common;

use std::path::Path;
use std::sync::Mutex;
use tempdir::TempDir;
use toolchain::fake::FakeRunner;
use toolchain::publish::{
    publish_with, Cause, GitIdentity, PollOptions, PublishOptions, StepId, StepState, StepUpdate,
    STEP_ORDER,
};
use toolchain::runner::{CancelToken, NoSleep, Output};

const TAG: &str = "v1.0.0";
const ASSET: &str = "night-run-1.0.0.g1rcart";

fn options(dir: &Path) -> PublishOptions {
    let mut options = PublishOptions::new(dir, "bryanthaboi", "night-run");
    options.description = Some("A short run through Kanto at night".to_string());
    options.commit_message = "Add night-run 1.0.0".to_string();
    options.poll = PollOptions {
        interval: std::time::Duration::from_millis(0),
        attempts: 3,
    };
    options
}

/// Everything the happy path needs; a test narrows one rule to force a failure.
fn scripted() -> FakeRunner {
    let fake = FakeRunner::new();
    fake.on("git", &["init"], Output::ok(""));
    fake.on(
        "git",
        &["config", "--get", "user.name"],
        Output::ok("Cart Author\n"),
    );
    fake.on(
        "git",
        &["config", "--get", "user.email"],
        Output::ok("author@example.com\n"),
    );
    fake.on("git", &["add"], Output::ok(""));
    fake.on_seq(
        "git",
        &["status", "--porcelain"],
        vec![Output::ok("A  cart.json\n"), Output::ok("")],
    );
    fake.on(
        "git",
        &["commit"],
        Output::ok("[main abc1234] Add night-run 1.0.0"),
    );
    fake.on(
        "git",
        &["remote", "get-url", "origin"],
        Output::fail(2, "error: No such remote 'origin'"),
    );
    fake.on(
        "gh",
        &["repo", "create"],
        Output::ok("https://github.com/bryanthaboi/night-run"),
    );
    fake.on("git", &["tag", "--list"], Output::ok(""));
    fake.on("git", &["tag"], Output::ok(""));
    fake.on("git", &["push", "origin"], Output::ok(""));
    fake.on(
        "gh",
        &["run", "list"],
        Output::ok(common::run_list(TAG, 42)),
    );
    fake.on_seq(
        "gh",
        &["run", "view"],
        vec![
            Output::ok(common::run_view("in_progress", "")),
            Output::ok(common::run_view("completed", "success")),
        ],
    );
    fake.on(
        "gh",
        &["release", "view"],
        Output::ok(common::release_view(TAG, &[ASSET, "sha256sums.txt"])),
    );
    fake
}

fn record() -> Mutex<Vec<(StepId, StepState)>> {
    Mutex::new(Vec::new())
}

fn sink(log: &Mutex<Vec<(StepId, StepState)>>) -> impl Fn(&StepUpdate) + '_ {
    move |update: &StepUpdate| {
        log.lock().expect("log").push((update.step, update.state));
    }
}

#[test]
fn the_happy_path_creates_tags_watches_and_confirms() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    let log = record();

    let outcome = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect("publish");

    assert_eq!(outcome.slug, "bryanthaboi/night-run");
    assert_eq!(outcome.repo_url, "https://github.com/bryanthaboi/night-run");
    assert_eq!(outcome.tag, TAG);
    assert_eq!(outcome.asset, ASSET);
    assert_eq!(outcome.run_id, Some(42));
    assert_eq!(
        outcome.install_url.as_deref(),
        Some("https://github.com/bryanthaboi/night-run/releases/download/v1.0.0/night-run-1.0.0.g1rcart")
    );
    assert!(dir.path().join(".github/workflows/release.yml").is_file());
    assert!(dir.path().join("label.layers.json").is_file());

    let done: Vec<StepId> = outcome.steps.iter().map(|step| step.step).collect();
    assert_eq!(done, STEP_ORDER.to_vec());
    assert!(outcome
        .steps
        .iter()
        .all(|step| matches!(step.state, StepState::Done | StepState::Skipped)));

    let updates = log.lock().expect("log").clone();
    assert_eq!(
        updates
            .iter()
            .filter(|(_, state)| *state == StepState::Pending)
            .count(),
        STEP_ORDER.len(),
        "every step is announced before the first one runs"
    );
    assert!(updates.contains(&(StepId::ConfirmAsset, StepState::Done)));
}

#[test]
fn every_step_passes_an_exact_argument_array() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    let log = record();
    publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect("publish");

    assert_eq!(
        fake.find("git", &["init"]).unwrap(),
        vec!["git", "init", "-b", "main"]
    );
    assert_eq!(
        fake.find("git", &["add"]).unwrap(),
        vec!["git", "add", "-A"]
    );
    assert_eq!(
        fake.find("git", &["commit"]).unwrap(),
        vec!["git", "commit", "-m", "Add night-run 1.0.0"]
    );
    assert_eq!(
        fake.find("gh", &["repo", "create"]).unwrap(),
        vec![
            "gh",
            "repo",
            "create",
            "bryanthaboi/night-run",
            "--public",
            "--description",
            "A short run through Kanto at night",
            "--source",
            ".",
            "--remote",
            "origin",
            "--push"
        ]
    );
    assert_eq!(
        fake.find("git", &["tag", "v1.0.0"]).unwrap(),
        vec!["git", "tag", "v1.0.0"]
    );
    assert_eq!(
        fake.find("git", &["push", "origin"]).unwrap(),
        vec!["git", "push", "origin", "v1.0.0"]
    );
    assert_eq!(
        fake.find("gh", &["run", "list"]).unwrap(),
        vec![
            "gh",
            "run",
            "list",
            "--repo",
            "bryanthaboi/night-run",
            "--branch",
            "v1.0.0",
            "--json",
            "databaseId,status,conclusion,headBranch,event,name",
            "--limit",
            "20"
        ]
    );
    assert_eq!(
        fake.find("gh", &["run", "view"]).unwrap(),
        vec![
            "gh",
            "run",
            "view",
            "42",
            "--repo",
            "bryanthaboi/night-run",
            "--json",
            "status,conclusion,jobs,url"
        ]
    );
    assert_eq!(
        fake.find("gh", &["release", "view"]).unwrap(),
        vec![
            "gh",
            "release",
            "view",
            "v1.0.0",
            "--repo",
            "bryanthaboi/night-run",
            "--json",
            "tagName,url,assets"
        ]
    );
    assert!(
        fake.argv_log().iter().all(|argv| !argv
            .iter()
            .any(|arg| arg == "-c" || arg == "sh" || arg.contains('\''))),
        "nothing is ever handed to a shell"
    );
}

#[test]
fn a_private_repo_is_created_private() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    let log = record();
    let mut options = options(dir.path());
    options.private = true;
    options.description = None;
    let outcome =
        publish_with(&fake, &CancelToken::new(), &options, &sink(&log), &NoSleep).expect("publish");
    assert!(outcome.private);
    assert_eq!(
        fake.find("gh", &["repo", "create"]).unwrap(),
        vec![
            "gh",
            "repo",
            "create",
            "bryanthaboi/night-run",
            "--private",
            "--source",
            ".",
            "--remote",
            "origin",
            "--push"
        ]
    );
}

#[test]
fn a_taken_repo_name_names_the_step_and_the_cause() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    fake.on(
        "gh",
        &["repo", "create"],
        Output::fail(
            1,
            "GraphQL: Name already exists on this account (createRepository)",
        ),
    );
    let log = record();
    let problem = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect_err("taken");
    assert_eq!(problem.step, Some(StepId::RepoCreate));
    assert_eq!(problem.step_id, Some("repo_create"));
    assert_eq!(problem.cause, Cause::RepoNameTaken);
    assert!(problem.stderr.contains("Name already exists"));
    assert!(problem.hint.contains("pick another name"));
    assert!(
        fake.find("git", &["tag", "v1.0.0"]).is_none(),
        "the pipeline stops at the failure"
    );
}

#[test]
fn an_expired_credential_is_told_apart_from_a_taken_name() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    fake.on(
        "gh",
        &["repo", "create"],
        Output::fail(
            1,
            "HTTP 401: Bad credentials (https://api.github.com/user/repos)",
        ),
    );
    let log = record();
    let problem = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect_err("auth");
    assert_eq!(problem.cause, Cause::AuthExpired);
}

#[test]
fn a_network_failure_is_reported_as_one() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    fake.on(
        "git",
        &["push", "origin"],
        Output::fail(
            128,
            "fatal: unable to access 'https://github.com/': Could not resolve host: github.com",
        ),
    );
    let log = record();
    let problem = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect_err("network");
    assert_eq!(problem.step, Some(StepId::PushTag));
    assert_eq!(problem.cause, Cause::Network);
}

#[test]
fn an_existing_tag_at_another_commit_stops_the_run() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    fake.on("git", &["tag", "--list"], Output::ok("v1.0.0\n"));
    fake.on("git", &["rev-list"], Output::ok("aaaaaaa\n"));
    fake.on("git", &["rev-parse", "HEAD"], Output::ok("bbbbbbb\n"));
    let log = record();
    let problem = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect_err("tag");
    assert_eq!(problem.step, Some(StepId::Tag));
    assert_eq!(problem.cause, Cause::TagExists);
}

#[test]
fn an_existing_tag_at_this_commit_is_skipped_not_recreated() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    fake.on("git", &["tag", "--list"], Output::ok("v1.0.0\n"));
    fake.on("git", &["rev-list"], Output::ok("aaaaaaa\n"));
    fake.on("git", &["rev-parse", "HEAD"], Output::ok("aaaaaaa\n"));
    let log = record();
    let outcome = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect("publish");
    let tag_step = outcome
        .steps
        .iter()
        .find(|step| step.step == StepId::Tag)
        .unwrap();
    assert_eq!(tag_step.state, StepState::Skipped);
    assert!(fake.find("git", &["tag", "v1.0.0"]).is_none());
}

#[test]
fn a_rejected_tag_push_is_a_tag_conflict() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    fake.on(
        "git",
        &["push", "origin"],
        Output::fail(1, "! [rejected] v1.0.0 -> v1.0.0 (already exists)"),
    );
    let log = record();
    let problem = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect_err("tag");
    assert_eq!(problem.cause, Cause::TagExists);
}

#[test]
fn actions_disabled_on_the_account_is_its_own_cause() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    fake.on(
        "gh",
        &["run", "list"],
        Output::fail(1, "HTTP 403: Actions is disabled for this repository"),
    );
    let log = record();
    let problem = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect_err("actions");
    assert_eq!(problem.step, Some(StepId::WatchRun));
    assert_eq!(problem.cause, Cause::ActionsDisabled);
}

#[test]
fn a_run_that_never_appears_is_reported_after_the_polls() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    fake.on("gh", &["run", "list"], Output::ok("[]"));
    let log = record();
    let problem = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect_err("missing");
    assert_eq!(problem.cause, Cause::RunMissing);
    assert_eq!(
        fake.count("gh", &["run", "list"]),
        3,
        "it polls the configured number of times"
    );
}

#[test]
fn a_failed_workflow_names_the_stage_that_failed() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    fake.on(
        "gh",
        &["run", "view"],
        Output::ok(common::run_view("completed", "failure")),
    );
    let log = record();
    let problem = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect_err("workflow");
    assert_eq!(problem.step, Some(StepId::WatchRun));
    assert_eq!(problem.cause, Cause::WorkflowFailed);
    assert!(problem.message.contains("validate --online"));
}

#[test]
fn a_release_without_the_bundle_is_caught() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    fake.on(
        "gh",
        &["release", "view"],
        Output::ok(common::release_view(TAG, &["sha256sums.txt"])),
    );
    let log = record();
    let problem = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect_err("asset");
    assert_eq!(problem.step, Some(StepId::ConfirmAsset));
    assert_eq!(problem.cause, Cause::AssetMissing);
    assert!(problem.message.contains(ASSET));
    assert!(problem.stderr.contains("sha256sums.txt"));
}

#[test]
fn a_cancelled_run_stops_at_the_step_it_reached() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let cancel = CancelToken::new();
    let fake = scripted();
    fake.on_cancel("git", &["add"], &cancel);
    let log = record();
    let problem = publish_with(&fake, &cancel, &options(dir.path()), &sink(&log), &NoSleep)
        .expect_err("cancel");
    assert_eq!(problem.cause, Cause::Cancelled);
    assert_eq!(problem.step, Some(StepId::GitAdd));
    assert!(fake.find("git", &["commit"]).is_none());
    assert!(problem
        .steps
        .iter()
        .any(|step| step.state == StepState::Failed));
}

#[test]
fn a_dirty_tree_at_tag_time_refuses_the_tag() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    fake.on(
        "git",
        &["status", "--porcelain"],
        Output::ok(" M cart.json\n"),
    );
    let log = record();
    let problem = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect_err("dirty");
    assert_eq!(problem.step, Some(StepId::Tag));
    assert_eq!(problem.cause, Cause::DirtyTree);
    assert!(problem.stderr.contains("cart.json"));
}

#[test]
fn no_git_identity_stops_before_the_commit() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    fake.on(
        "git",
        &["config", "--get", "user.email"],
        Output::fail(1, ""),
    );
    let log = record();
    let problem = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect_err("identity");
    assert_eq!(problem.step, Some(StepId::GitInit));
    assert_eq!(problem.cause, Cause::NoGitIdentity);
    assert!(fake.find("git", &["commit"]).is_none());
}

#[test]
fn an_offered_identity_is_written_to_the_local_config() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    fake.on(
        "git",
        &["config", "--get", "user.name"],
        Output::fail(1, ""),
    );
    fake.on(
        "git",
        &["config", "--get", "user.email"],
        Output::fail(1, ""),
    );
    fake.on("git", &["config", "--local"], Output::ok(""));
    let mut options = options(dir.path());
    options.identity = Some(GitIdentity {
        name: "Cart Author".to_string(),
        email: "author@example.com".to_string(),
    });
    let log = record();
    publish_with(&fake, &CancelToken::new(), &options, &sink(&log), &NoSleep).expect("publish");
    assert_eq!(
        fake.find("git", &["config", "--local"]).unwrap(),
        vec!["git", "config", "--local", "user.name", "Cart Author"]
    );
}

#[test]
fn an_existing_repository_and_origin_are_reused() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    std::fs::create_dir_all(dir.path().join(".git")).expect("git dir");
    let fake = scripted();
    fake.on(
        "git",
        &["remote", "get-url", "origin"],
        Output::ok("https://github.com/bryanthaboi/night-run.git\n"),
    );
    fake.on("git", &["push", "-u", "origin", "main"], Output::ok(""));
    let log = record();
    let outcome = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect("publish");

    let state = |id: StepId| {
        outcome
            .steps
            .iter()
            .find(|step| step.step == id)
            .unwrap()
            .state
    };
    assert_eq!(state(StepId::GitInit), StepState::Skipped);
    assert_eq!(state(StepId::RepoCreate), StepState::Skipped);
    assert!(fake.find("gh", &["repo", "create"]).is_none());
    assert_eq!(
        fake.find("git", &["push", "-u"]).unwrap(),
        vec!["git", "push", "-u", "origin", "main"]
    );
    assert!(fake.find("git", &["init"]).is_none());
}

#[test]
fn an_already_stamped_workflow_is_left_alone() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let marker = "# hand edited\n";
    let path = dir.path().join(".github/workflows/release.yml");
    let body = format!(
        "{}{}",
        marker,
        std::fs::read_to_string(&path).expect("read")
    );
    std::fs::write(&path, &body).expect("write");
    let fake = scripted();
    let log = record();
    let outcome = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect("publish");
    let state = outcome
        .steps
        .iter()
        .find(|step| step.step == StepId::WriteWorkflow)
        .unwrap()
        .state;
    assert_eq!(state, StepState::Skipped);
    assert!(std::fs::read_to_string(&path)
        .expect("read")
        .starts_with(marker));
}

#[test]
fn strict_validation_runs_before_any_command() {
    let dir = TempDir::new("cart").expect("temp");
    let mut cart = common::make_cart(dir.path());
    cart.insert("seal".to_string(), serde_json::json!("melted"));
    cartcore::cart::write_cart(dir.path(), &cart).expect("write");
    let fake = scripted();
    let log = record();
    let problem = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect_err("validation");
    assert_eq!(problem.cause, Cause::Validation);
    assert!(problem.step.is_none());
    assert!(!problem.findings.is_empty());
    assert!(
        fake.calls().is_empty(),
        "nothing runs until the cart validates"
    );
}

#[test]
fn a_missing_tool_is_reported_as_a_missing_tool() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    fake.missing("gh");
    let log = record();
    let problem = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect_err("gh");
    assert_eq!(problem.step, Some(StepId::RepoCreate));
    assert_eq!(problem.cause, Cause::ToolMissing);
    assert!(problem.message.contains("gh"));
}

#[test]
fn a_cart_directory_with_a_windows_shaped_name_is_passed_through_verbatim() {
    let root = TempDir::new("cart").expect("temp");
    let dir = root.path().join(r"Cart Projects\night run");
    std::fs::create_dir_all(&dir).expect("dir");
    common::make_cart(&dir);
    let fake = scripted();
    let log = record();
    publish_with(
        &fake,
        &CancelToken::new(),
        &options(&dir),
        &sink(&log),
        &NoSleep,
    )
    .expect("publish");

    let cwd = fake.calls()[0].cwd.clone().expect("cwd");
    assert_eq!(cwd, dir);
    let shown = cwd.to_string_lossy();
    assert!(shown.contains(r"Cart Projects\night run"));
    assert!(
        !shown.contains('"') && !shown.contains('\''),
        "a path is never quoted"
    );
}

#[test]
fn the_step_log_keeps_every_command_with_its_streams() {
    let dir = TempDir::new("cart").expect("temp");
    common::make_cart(dir.path());
    let fake = scripted();
    let log = record();
    let outcome = publish_with(
        &fake,
        &CancelToken::new(),
        &options(dir.path()),
        &sink(&log),
        &NoSleep,
    )
    .expect("publish");
    let commit = outcome
        .steps
        .iter()
        .find(|step| step.step == StepId::GitCommit)
        .unwrap();
    let logged = commit.commands.last().unwrap();
    assert_eq!(
        logged.argv,
        vec!["git", "commit", "-m", "Add night-run 1.0.0"]
    );
    assert_eq!(logged.code, Some(0));
    assert!(logged.stdout.contains("[main abc1234]"));
    assert_eq!(
        logged.cwd.as_deref(),
        Some(dir.path().to_string_lossy().as_ref())
    );
}
