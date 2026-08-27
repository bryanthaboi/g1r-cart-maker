use std::path::{Path, PathBuf};
use toolchain::detect::{
    self, dir_identity, gh_auth_status, global_identity, parse_auth_status, parse_version,
    set_identity, which_in, Credential, TokenEnv,
};
use toolchain::fake::FakeRunner;
use toolchain::runner::{CancelToken, Output};

const NEW_STATUS: &str = "github.com\n  ✓ Logged in to github.com account bryanthaboi (keyring)\n  - Active account: true\n  - Git operations protocol: ssh\n  - Token: gho_0123456789abcdefghijklmnopqrstuvwxyz\n  - Token scopes: 'gist', 'read:org', 'repo'\n";
const OLD_STATUS: &str = "github.com\n  ✓ Logged in to github.com as bryanthaboi (oauth_token)\n  ✓ Git operations for github.com configured to use https protocol.\n  ✓ Token: *******************\n";
const TOKEN_STATUS: &str = "github.com\n  ✓ Logged in to github.com account ci-bot (GITHUB_TOKEN)\n  - Git operations protocol: https\n";
const LOGGED_OUT: &str =
    "You are not logged into any GitHub hosts. To log in, run: gh auth login\n";

#[test]
fn a_missing_git_is_reported_as_missing() {
    let fake = FakeRunner::new();
    fake.missing("git");
    fake.on(
        "gh",
        &["--version"],
        Output::ok("gh version 2.62.0 (2024-11-14)"),
    );
    let found = detect::detect(&fake, &CancelToken::new());
    assert!(!found.git.found);
    assert!(found.git.version.is_none());
    assert!(found
        .git
        .detail
        .as_deref()
        .unwrap_or_default()
        .contains("not on PATH"));
    assert!(!found.ready());
    assert_eq!(fake.argv_log()[0], vec!["git", "--version"]);
}

#[test]
fn a_missing_gh_is_reported_as_missing() {
    let fake = FakeRunner::new();
    fake.on("git", &["--version"], Output::ok("git version 2.43.0"));
    fake.missing("gh");
    let found = detect::detect(&fake, &CancelToken::new());
    assert!(found.git.found);
    assert_eq!(found.git.version.as_deref(), Some("2.43.0"));
    assert!(!found.gh.found);
    assert!(!found.ready());
}

#[test]
fn versions_come_out_of_the_tools_own_first_line() {
    assert_eq!(
        parse_version("git version 2.39.3 (Apple Git-145)").as_deref(),
        Some("2.39.3")
    );
    assert_eq!(
        parse_version("gh version 2.62.0 (2024-11-14)").as_deref(),
        Some("2.62.0")
    );
    assert_eq!(parse_version("no version here"), None);
}

#[test]
fn a_tool_that_runs_but_fails_is_not_found() {
    let fake = FakeRunner::new();
    fake.on("git", &["--version"], Output::fail(127, "bad interpreter"));
    let status = detect::detect_tool(&fake, &CancelToken::new(), "git");
    assert!(!status.found);
}

#[test]
fn an_authenticated_gh_reports_its_account_and_protocol() {
    let fake = FakeRunner::new();
    fake.on("gh", &["auth", "status"], Output::ok(NEW_STATUS));
    let status = gh_auth_status(&fake, &CancelToken::new(), TokenEnv::none()).expect("run");
    assert!(status.authenticated);
    assert_eq!(status.account.as_deref(), Some("bryanthaboi"));
    assert_eq!(status.protocol.as_deref(), Some("ssh"));
    assert_eq!(status.host.as_deref(), Some("github.com"));
    assert_eq!(status.scopes, vec!["gist", "read:org", "repo"]);
    assert_eq!(status.credential, Credential::Stored);
    assert!(!status
        .detail
        .contains("gho_0123456789abcdefghijklmnopqrstuvwxyz"));
    assert_eq!(fake.argv_log(), vec![vec!["gh", "auth", "status"]]);
}

#[test]
fn the_older_gh_wording_still_parses() {
    let status = parse_auth_status(OLD_STATUS, true, TokenEnv::none());
    assert!(status.authenticated);
    assert_eq!(status.account.as_deref(), Some("bryanthaboi"));
    assert_eq!(status.protocol.as_deref(), Some("https"));
}

#[test]
fn an_unauthenticated_gh_says_which_command_fixes_it() {
    let fake = FakeRunner::new();
    fake.on("gh", &["auth", "status"], Output::fail(1, LOGGED_OUT));
    let status = gh_auth_status(&fake, &CancelToken::new(), TokenEnv::none()).expect("run");
    assert!(!status.authenticated);
    assert_eq!(status.credential, Credential::None);
    assert!(status.credential_note.contains("gh auth login"));
}

#[test]
fn a_token_in_the_environment_wins_and_is_named() {
    let fake = FakeRunner::new();
    fake.on("gh", &["auth", "status"], Output::ok(TOKEN_STATUS));
    let status = gh_auth_status(&fake, &CancelToken::new(), TokenEnv::none()).expect("run");
    assert_eq!(
        status.credential,
        Credential::Environment {
            variable: "GITHUB_TOKEN".to_string()
        }
    );
    assert!(status.credential_note.contains("GITHUB_TOKEN"));
    assert_eq!(status.account.as_deref(), Some("ci-bot"));
}

#[test]
fn gh_token_beats_github_token_beats_the_stored_login() {
    let both = TokenEnv {
        gh_token: true,
        github_token: true,
    };
    assert_eq!(both.winner(), Some("GH_TOKEN"));
    assert_eq!(
        TokenEnv {
            gh_token: false,
            github_token: true
        }
        .winner(),
        Some("GITHUB_TOKEN")
    );
    assert_eq!(TokenEnv::none().winner(), None);

    let status = parse_auth_status(NEW_STATUS, true, both);
    assert_eq!(
        status.credential,
        Credential::Environment {
            variable: "GH_TOKEN".to_string()
        }
    );
}

#[test]
fn the_git_identity_is_read_globally_and_per_directory() {
    let fake = FakeRunner::new();
    fake.on(
        "git",
        &["config", "--global", "--get", "user.name"],
        Output::ok("Global Name\n"),
    );
    fake.on(
        "git",
        &["config", "--global", "--get", "user.email"],
        Output::ok("global@example.com\n"),
    );
    fake.on(
        "git",
        &["config", "--get", "user.name"],
        Output::ok("Local Name\n"),
    );
    fake.on(
        "git",
        &["config", "--get", "user.email"],
        Output::fail(1, ""),
    );
    let cancel = CancelToken::new();

    let global = global_identity(&fake, &cancel);
    assert_eq!(global.name.as_deref(), Some("Global Name"));
    assert!(global.complete());

    let dir = Path::new("/tmp/cart");
    let local = dir_identity(&fake, &cancel, dir);
    assert_eq!(local.name.as_deref(), Some("Local Name"));
    assert_eq!(local.email, None);
    assert!(!local.complete());
    assert_eq!(
        fake.calls()[2].cwd,
        Some(PathBuf::from("/tmp/cart")),
        "a per-directory read runs in that directory"
    );
}

#[test]
fn setting_an_identity_writes_the_local_config_only() {
    let fake = FakeRunner::new();
    fake.on("git", &["config", "--local"], Output::ok(""));
    let dir = Path::new("/tmp/cart");
    set_identity(
        &fake,
        &CancelToken::new(),
        dir,
        "Cart Author",
        "author@example.com",
    )
    .expect("set");
    assert_eq!(
        fake.argv_log(),
        vec![
            vec!["git", "config", "--local", "user.name", "Cart Author"],
            vec![
                "git",
                "config",
                "--local",
                "user.email",
                "author@example.com"
            ],
        ]
    );
    assert!(fake
        .calls()
        .iter()
        .all(|call| call.cwd == Some(dir.to_path_buf())));
    assert!(
        fake.argv_log()
            .iter()
            .all(|argv| !argv.contains(&"--global".to_string())),
        "the user's global identity is never touched"
    );
}

#[test]
fn a_rejected_config_write_carries_the_tools_message() {
    let fake = FakeRunner::new();
    fake.on(
        "git",
        &["config", "--local"],
        Output::fail(4, "could not lock config file"),
    );
    let problem = set_identity(
        &fake,
        &CancelToken::new(),
        Path::new("/tmp/cart"),
        "Cart Author",
        "author@example.com",
    )
    .expect_err("rejected");
    assert!(problem.to_string().contains("could not lock config file"));
}

#[test]
fn path_lookup_walks_path_and_windows_extensions() {
    let separator = if cfg!(windows) { ";" } else { ":" };
    let path = ["/one", "/two"].join(separator);
    let found = which_in(&path, ".COM;.EXE", "gh", &|candidate: &Path| {
        candidate == Path::new("/two/gh.EXE") || candidate == Path::new("/two/gh")
    });
    assert_eq!(found, Some(PathBuf::from("/two/gh")));

    let windows_only = which_in(&path, ".COM;.EXE", "gh", &|candidate: &Path| {
        candidate == Path::new("/two/gh.EXE")
    });
    assert_eq!(windows_only, Some(PathBuf::from("/two/gh.EXE")));
    assert_eq!(which_in("", ".EXE", "gh", &|_| true), None);
}

const FAKE_TOKEN: &str = "gho_0123456789abcdefghijklmnopqrstuvwxyz";

#[test]
fn gh_hands_over_the_credential_it_already_holds() {
    assert_eq!(
        detect::token_from(true, &format!("{}\n", FAKE_TOKEN)).as_deref(),
        Some(FAKE_TOKEN)
    );
}

#[test]
fn a_signed_out_gh_yields_no_credential() {
    assert_eq!(detect::token_from(false, "no oauth token found"), None);
}

#[test]
fn a_blank_line_is_not_a_credential() {
    assert_eq!(detect::token_from(true, "   \n"), None);
}

/// The token must never reach a log the window or a bug report can show.
#[test]
fn a_credential_is_masked_by_the_log_redactor() {
    let masked = toolchain::runner::redact(&format!("using {} for the api", FAKE_TOKEN));
    assert!(!masked.contains(FAKE_TOKEN), "got {}", masked);
    assert!(masked.contains("gho_***"));
}
