//! Finding git and gh, reading their versions, and answering the two questions
//! a publish depends on: which GitHub credential wins, and who the commit is by.

use crate::runner::{CancelToken, Invocation, RunError, Runner};
use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};

pub const GIT: &str = "git";
pub const GH: &str = "gh";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolStatus {
    pub program: String,
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    /// The tool's own first line, or the reason it could not be run.
    pub detail: Option<String>,
}

impl ToolStatus {
    fn missing(program: &str, detail: String) -> Self {
        Self {
            program: program.to_string(),
            found: false,
            path: which(program).map(|path| path.display().to_string()),
            version: None,
            detail: Some(detail),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Toolchain {
    pub git: ToolStatus,
    pub gh: ToolStatus,
}

impl Toolchain {
    pub fn ready(&self) -> bool {
        self.git.found && self.gh.found
    }
}

pub fn detect(runner: &dyn Runner, cancel: &CancelToken) -> Toolchain {
    Toolchain {
        git: detect_tool(runner, cancel, GIT),
        gh: detect_tool(runner, cancel, GH),
    }
}

pub fn detect_tool(runner: &dyn Runner, cancel: &CancelToken, program: &str) -> ToolStatus {
    let invocation = Invocation::new(program, ["--version"]);
    match runner.run(&invocation, cancel) {
        Err(problem) => ToolStatus::missing(program, problem.to_string()),
        Ok(output) if !output.success() => {
            ToolStatus::missing(program, first_line(&output.combined()))
        }
        Ok(output) => {
            let line = first_line(&output.combined());
            ToolStatus {
                program: program.to_string(),
                found: true,
                path: which(program).map(|path| path.display().to_string()),
                version: parse_version(&line),
                detail: Some(line),
            }
        }
    }
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().trim().to_string()
}

/// `git version 2.43.0`, `gh version 2.62.0 (2024-11-14)`.
pub fn parse_version(line: &str) -> Option<String> {
    line.split_whitespace()
        .map(|token| token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '.'))
        .find(|token| {
            token.contains('.')
                && token.starts_with(|ch: char| ch.is_ascii_digit())
                && token
                    .chars()
                    .all(|ch| ch.is_ascii_digit() || ch == '.' || ch.is_ascii_alphabetic())
        })
        .map(str::to_string)
}

pub fn which(program: &str) -> Option<PathBuf> {
    let path = env::var("PATH").ok()?;
    let extensions = env::var("PATHEXT").unwrap_or_default();
    which_in(&path, &extensions, program, &|candidate| {
        candidate.is_file()
    })
}

/// PATH lookup with the filesystem injected, so it is testable and never shells out.
pub fn which_in(
    path_var: &str,
    path_ext: &str,
    program: &str,
    exists: &dyn Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let separator = if cfg!(windows) { ';' } else { ':' };
    let mut suffixes: Vec<String> = vec![String::new()];
    for ext in path_ext.split(';') {
        if !ext.trim().is_empty() {
            suffixes.push(ext.trim().to_string());
        }
    }
    for dir in path_var.split(separator) {
        if dir.is_empty() {
            continue;
        }
        for suffix in &suffixes {
            let candidate = Path::new(dir).join(format!("{}{}", program, suffix));
            if exists(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

// ------- gh authentication

/// Which credential gh will use. The environment beats the stored login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Credential {
    /// A token in the environment, named by the variable that holds it.
    Environment {
        variable: String,
    },
    /// gh's own stored login (keyring or hosts.yml).
    Stored,
    None,
}

/// Presence only. The value is gh's business and is never read here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct TokenEnv {
    pub gh_token: bool,
    pub github_token: bool,
}

impl TokenEnv {
    pub fn from_env() -> Self {
        Self {
            gh_token: has_value("GH_TOKEN"),
            github_token: has_value("GITHUB_TOKEN"),
        }
    }

    pub fn none() -> Self {
        Self::default()
    }

    /// GH_TOKEN wins over GITHUB_TOKEN; either wins over the stored login.
    pub fn winner(&self) -> Option<&'static str> {
        if self.gh_token {
            Some("GH_TOKEN")
        } else if self.github_token {
            Some("GITHUB_TOKEN")
        } else {
            None
        }
    }
}

fn has_value(name: &str) -> bool {
    env::var(name)
        .map(|value| !value.is_empty())
        .unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub host: Option<String>,
    pub account: Option<String>,
    pub protocol: Option<String>,
    pub scopes: Vec<String>,
    pub credential: Credential,
    /// Which variable gh named itself, when it did.
    pub credential_note: String,
    pub detail: String,
}

pub fn gh_auth_status(
    runner: &dyn Runner,
    cancel: &CancelToken,
    token_env: TokenEnv,
) -> Result<AuthStatus, RunError> {
    let output = runner.run(&Invocation::new(GH, ["auth", "status"]), cancel)?;
    Ok(parse_auth_status(
        &crate::runner::redact(&output.combined()),
        output.success(),
        token_env,
    ))
}

/// The credential `gh` already holds, so API calls are authenticated without
/// anyone setting GITHUB_TOKEN by hand.
///
/// This spawns `gh` directly instead of going through `Runner`, which redacts
/// every token out of the output it returns: a redacted token is exactly what
/// this needs unredacted. Nothing here may be logged, shown, or put in an
/// error; only the caller handing it to the HTTP client should ever see it.
pub fn gh_token() -> Option<String> {
    let output = std::process::Command::new(GH)
        .args(["auth", "token"])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    token_from(
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
    )
}

/// Split out so the shape can be tested without a real `gh`.
pub fn token_from(success: bool, stdout: &str) -> Option<String> {
    if !success {
        return None;
    }
    let token = stdout.trim();
    if token.is_empty() {
        return None;
    }
    Some(token.to_string())
}

pub fn parse_auth_status(text: &str, success: bool, token_env: TokenEnv) -> AuthStatus {
    let mut host = None;
    let mut account = None;
    let mut protocol = None;
    let mut scopes = Vec::new();
    let mut named_env: Option<String> = None;
    let mut logged_in = false;

    for raw in text.lines() {
        let line = raw.trim();
        let bare = line
            .trim_start_matches(['✓', '✗', 'x', 'X', '-', '*', ' '])
            .trim();
        if !line.starts_with(['✓', '✗', '-', '*']) && line.ends_with(".com") && !line.contains(' ')
        {
            host = Some(line.to_string());
        }
        if let Some(rest) = bare.strip_prefix("Logged in to ") {
            logged_in = true;
            let mut words = rest.split_whitespace();
            if let Some(found) = words.next() {
                host = Some(found.to_string());
            }
            let tail: Vec<&str> = words.collect();
            let mut at = 0;
            while at < tail.len() {
                if tail[at] == "account" || tail[at] == "as" {
                    if let Some(name) = tail.get(at + 1) {
                        account = Some(name.trim_matches(['(', ')']).to_string());
                    }
                    break;
                }
                at += 1;
            }
            if let Some(open) = rest.rfind('(') {
                let inside = rest[open + 1..].trim_end_matches([')', '.']).trim();
                if inside == "GH_TOKEN" || inside == "GITHUB_TOKEN" {
                    named_env = Some(inside.to_string());
                }
            }
        }
        if let Some(rest) = bare.strip_prefix("Git operations protocol:") {
            protocol = Some(rest.trim().to_string());
        } else if bare.starts_with("Git operations for") {
            if let Some(at) = bare.find("use ") {
                protocol = Some(
                    bare[at + 4..]
                        .trim()
                        .trim_end_matches("protocol.")
                        .trim()
                        .to_string(),
                );
            }
        }
        if let Some(rest) = bare.strip_prefix("Token scopes:") {
            scopes = rest
                .split(',')
                .map(|scope| scope.trim().trim_matches(['\'', '"']).to_string())
                .filter(|scope| !scope.is_empty())
                .collect();
        }
    }

    let authenticated = success && logged_in;
    let environment = named_env.or_else(|| token_env.winner().map(str::to_string));
    let credential = match (&environment, authenticated) {
        (Some(variable), _) => Credential::Environment {
            variable: variable.clone(),
        },
        (None, true) => Credential::Stored,
        (None, false) => Credential::None,
    };
    let credential_note = match &credential {
        Credential::Environment { variable } => format!(
            "{} is set in the environment; gh will use it instead of any stored login",
            variable
        ),
        Credential::Stored => "gh will use its own stored login".to_string(),
        Credential::None => "no GitHub credential; run `gh auth login`".to_string(),
    };

    AuthStatus {
        authenticated,
        host,
        account,
        protocol,
        scopes,
        credential,
        credential_note,
        detail: text.trim().to_string(),
    }
}

// ------- git identity

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Identity {
    pub name: Option<String>,
    pub email: Option<String>,
}

impl Identity {
    pub fn complete(&self) -> bool {
        self.name.is_some() && self.email.is_some()
    }
}

pub fn global_identity(runner: &dyn Runner, cancel: &CancelToken) -> Identity {
    Identity {
        name: config_value(runner, cancel, None, &["--global", "--get", "user.name"]),
        email: config_value(runner, cancel, None, &["--global", "--get", "user.email"]),
    }
}

/// The identity a commit in `dir` would actually carry: local over global.
pub fn dir_identity(runner: &dyn Runner, cancel: &CancelToken, dir: &Path) -> Identity {
    Identity {
        name: config_value(runner, cancel, Some(dir), &["--get", "user.name"]),
        email: config_value(runner, cancel, Some(dir), &["--get", "user.email"]),
    }
}

fn config_value(
    runner: &dyn Runner,
    cancel: &CancelToken,
    dir: Option<&Path>,
    args: &[&str],
) -> Option<String> {
    let mut argv = vec!["config".to_string()];
    argv.extend(args.iter().map(|arg| arg.to_string()));
    let mut invocation = Invocation::new(GIT, argv);
    if let Some(dir) = dir {
        invocation = invocation.in_dir(dir);
    }
    let output = runner.run(&invocation, cancel).ok()?;
    if !output.success() {
        return None;
    }
    let value = output.stdout.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdentityError {
    #[error(transparent)]
    Run(#[from] RunError),
    #[error("{argv}: {stderr}")]
    Rejected { argv: String, stderr: String },
}

/// Writes the LOCAL config of `dir` only; the user's global identity is untouched.
pub fn set_identity(
    runner: &dyn Runner,
    cancel: &CancelToken,
    dir: &Path,
    name: &str,
    email: &str,
) -> Result<(), IdentityError> {
    for (key, value) in [("user.name", name), ("user.email", email)] {
        let invocation = Invocation::new(GIT, ["config", "--local", key, value]).in_dir(dir);
        let output = runner.run(&invocation, cancel)?;
        if !output.success() {
            return Err(IdentityError::Rejected {
                argv: invocation.argv().join(" "),
                stderr: output.problem(),
            });
        }
    }
    Ok(())
}
