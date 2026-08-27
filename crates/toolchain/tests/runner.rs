use std::path::PathBuf;
use std::time::Duration;
use toolchain::fake::FakeRunner;
use toolchain::runner::{
    redact, CancelToken, Invocation, NoSleep, Output, RunError, Runner, Sleeper, SystemRunner,
};

#[test]
fn an_invocation_is_an_argument_array() {
    let invocation = Invocation::new("git", ["commit", "-m", "a message with spaces"])
        .in_dir("/tmp/cart dir")
        .with_env("GIT_TERMINAL_PROMPT", "0");
    assert_eq!(
        invocation.argv(),
        vec!["git", "commit", "-m", "a message with spaces"]
    );
    assert_eq!(invocation.cwd, Some(PathBuf::from("/tmp/cart dir")));
    assert_eq!(
        invocation.env,
        vec![("GIT_TERMINAL_PROMPT".to_string(), "0".to_string())]
    );
}

#[test]
fn the_longest_matching_rule_wins() {
    let fake = FakeRunner::new();
    fake.on("git", &["tag"], Output::ok("created"));
    fake.on("git", &["tag", "--list"], Output::ok("v1.0.0"));
    let cancel = CancelToken::new();

    let listed = fake
        .run(
            &Invocation::new("git", ["tag", "--list", "v1.0.0"]),
            &cancel,
        )
        .expect("run");
    let made = fake
        .run(&Invocation::new("git", ["tag", "v1.0.0"]), &cancel)
        .expect("run");
    assert_eq!(listed.stdout, "v1.0.0");
    assert_eq!(made.stdout, "created");
    assert_eq!(
        fake.argv_log(),
        vec![
            vec!["git", "tag", "--list", "v1.0.0"],
            vec!["git", "tag", "v1.0.0"]
        ]
    );
}

#[test]
fn a_sequence_replays_then_repeats_its_last_result() {
    let fake = FakeRunner::new();
    fake.on_seq(
        "gh",
        &["run", "view"],
        vec![Output::ok("queued"), Output::ok("done")],
    );
    let cancel = CancelToken::new();
    let call = || {
        fake.run(&Invocation::new("gh", ["run", "view", "7"]), &cancel)
            .expect("run")
            .stdout
    };
    assert_eq!(call(), "queued");
    assert_eq!(call(), "done");
    assert_eq!(call(), "done");
}

#[test]
fn a_missing_program_reports_not_found() {
    let fake = FakeRunner::new();
    fake.missing("gh");
    let problem = fake
        .run(
            &Invocation::new("gh", ["auth", "status"]),
            &CancelToken::new(),
        )
        .expect_err("missing");
    assert_eq!(problem, RunError::NotFound("gh".to_string()));
}

#[test]
fn an_unscripted_call_is_an_error_not_a_silent_pass() {
    let fake = FakeRunner::new();
    let problem = fake
        .run(&Invocation::new("git", ["status"]), &CancelToken::new())
        .expect_err("unscripted");
    assert!(matches!(problem, RunError::Unscripted(_)));
}

#[test]
fn a_token_never_lands_in_a_captured_log() {
    let text = "  - Token: ghp_0123456789abcdefghijklmnopqrstuvwxyz\nusing github_pat_11ABCDEFG0123456789abcdefg in the url\n";
    let clean = redact(text);
    assert!(!clean.contains("ghp_0123456789abcdefghijklmnopqrstuvwxyz"));
    assert!(!clean.contains("github_pat_11ABCDEFG0123456789abcdefg"));
    assert!(clean.contains("Token: ***"));
    assert!(clean.contains("github_pat_***"));
}

#[test]
fn no_sleep_is_instant() {
    let start = std::time::Instant::now();
    NoSleep.sleep(Duration::from_secs(30));
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[cfg(unix)]
mod system {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn tool(dir: &std::path::Path, name: &str, body: &str) {
        let path = dir.join(name);
        fs::write(&path, body).expect("write tool");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    #[test]
    fn the_system_runner_passes_the_array_through_and_captures_both_streams() {
        let bin = tempdir::TempDir::new("bin").expect("temp");
        tool(
            bin.path(),
            "git",
            "#!/bin/sh\nprintf '%s\\n' \"$@\"\necho \"pwd=$(pwd)\" 1>&2\necho \"env=$G1R_TEST\" 1>&2\nexit 3\n",
        );
        let work = tempdir::TempDir::new("work").expect("temp");
        let invocation = Invocation::new(
            bin.path().join("git").to_string_lossy().as_ref(),
            ["commit", "-m", "two words; rm -rf /", "$(whoami)"],
        )
        .in_dir(work.path())
        .with_env("G1R_TEST", "overlaid");

        let output = SystemRunner::new()
            .run(&invocation, &CancelToken::new())
            .expect("spawn");
        assert_eq!(output.code, Some(3));
        assert_eq!(
            output.stdout.lines().collect::<Vec<_>>(),
            vec!["commit", "-m", "two words; rm -rf /", "$(whoami)"]
        );
        assert!(output.stderr.contains("env=overlaid"));
        assert!(!output.success());
    }

    #[test]
    fn a_program_that_is_not_there_is_not_found() {
        let problem = SystemRunner::new()
            .run(
                &Invocation::new("g1r-nonexistent-tool", ["--version"]),
                &CancelToken::new(),
            )
            .expect_err("missing");
        assert!(matches!(problem, RunError::NotFound(_)));
    }

    #[test]
    fn a_cancelled_child_is_killed() {
        let bin = tempdir::TempDir::new("bin").expect("temp");
        tool(bin.path(), "sleeper", "#!/bin/sh\nsleep 30\n");
        let cancel = CancelToken::new();
        let token = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(120));
            token.cancel();
        });
        let start = std::time::Instant::now();
        let output = SystemRunner::new()
            .run(
                &Invocation::new(
                    bin.path().join("sleeper").to_string_lossy().as_ref(),
                    [] as [&str; 0],
                ),
                &cancel,
            )
            .expect("spawn");
        assert!(output.cancelled);
        assert!(start.elapsed() < Duration::from_secs(10));
    }
}
