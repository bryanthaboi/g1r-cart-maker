//! Every subprocess goes through a Runner as an argument array, a working
//! directory and an environment overlay. Nothing here ever builds a shell string.

use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// A program plus its argv tail. `program` is never interpreted by a shell.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Invocation {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
}

impl Invocation {
    pub fn new<S: Into<String>, I: IntoIterator<Item = S>>(program: &str, args: I) -> Self {
        Self {
            program: program.to_string(),
            args: args.into_iter().map(Into::into).collect(),
            cwd: None,
            env: Vec::new(),
        }
    }

    pub fn in_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.cwd = Some(dir.as_ref().to_path_buf());
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Program first, then the arguments: the exact array handed to the OS.
    pub fn argv(&self) -> Vec<String> {
        let mut argv = Vec::with_capacity(self.args.len() + 1);
        argv.push(self.program.clone());
        argv.extend(self.args.iter().cloned());
        argv
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Output {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub cancelled: bool,
}

impl Output {
    pub fn ok(stdout: impl Into<String>) -> Self {
        Self {
            code: Some(0),
            stdout: stdout.into(),
            stderr: String::new(),
            cancelled: false,
        }
    }

    pub fn fail(code: i32, stderr: impl Into<String>) -> Self {
        Self {
            code: Some(code),
            stdout: String::new(),
            stderr: stderr.into(),
            cancelled: false,
        }
    }

    pub fn cancelled() -> Self {
        Self {
            code: None,
            stdout: String::new(),
            stderr: String::new(),
            cancelled: true,
        }
    }

    pub fn success(&self) -> bool {
        !self.cancelled && self.code == Some(0)
    }

    /// Both streams, for the tools that split their reporting across them.
    pub fn combined(&self) -> String {
        let mut text = self.stdout.clone();
        if !self.stderr.is_empty() {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&self.stderr);
        }
        text
    }

    /// stderr when the tool wrote one, otherwise whatever it said on stdout.
    pub fn problem(&self) -> String {
        if self.stderr.trim().is_empty() {
            self.stdout.trim().to_string()
        } else {
            self.stderr.trim().to_string()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    #[error("{0} is not installed, or not on PATH")]
    NotFound(String),
    #[error("{program} could not be started: {detail}")]
    Spawn { program: String, detail: String },
    #[error("no scripted result for `{0}`")]
    Unscripted(String),
}

/// Cooperative cancellation, shared with the UI thread that owns the button.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

pub trait Runner: Send + Sync {
    fn run(&self, invocation: &Invocation, cancel: &CancelToken) -> Result<Output, RunError>;
}

/// Polling waits between workflow polls; the tests swap in one that never waits.
pub trait Sleeper: Send + Sync {
    fn sleep(&self, duration: Duration);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemSleeper;

impl Sleeper for SystemSleeper {
    fn sleep(&self, duration: Duration) {
        thread::sleep(duration);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoSleep;

impl Sleeper for NoSleep {
    fn sleep(&self, _duration: Duration) {}
}

const POLL: Duration = Duration::from_millis(20);
/// How long a cancelled run waits for whatever output already arrived.
const DRAIN_GRACE: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemRunner;

impl SystemRunner {
    pub fn new() -> Self {
        Self
    }
}

impl Runner for SystemRunner {
    fn run(&self, invocation: &Invocation, cancel: &CancelToken) -> Result<Output, RunError> {
        let mut command = Command::new(&invocation.program);
        command.args(&invocation.args);
        if let Some(dir) = &invocation.cwd {
            command.current_dir(dir);
        }
        for (key, value) in &invocation.env {
            command.env(key, value);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        own_process_group(&mut command);

        // Putting the child in its own process group makes the platform report a
        // missing program as an ordinary exit 127 instead of a spawn error, so
        // the lookup happens here where the answer is unambiguous.
        if !program_exists(&invocation.program) {
            return Err(RunError::NotFound(invocation.program.clone()));
        }

        let mut child = command.spawn().map_err(|problem| {
            if problem.kind() == std::io::ErrorKind::NotFound {
                RunError::NotFound(invocation.program.clone())
            } else {
                RunError::Spawn {
                    program: invocation.program.clone(),
                    detail: problem.to_string(),
                }
            }
        })?;

        let out_pipe = child.stdout.take();
        let err_pipe = child.stderr.take();
        let (out_tx, out_rx) = std::sync::mpsc::channel();
        let (err_tx, err_rx) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let _ = out_tx.send(drain(out_pipe));
        });
        thread::spawn(move || {
            let _ = err_tx.send(drain(err_pipe));
        });

        let mut cancelled = false;
        let status = loop {
            if cancel.is_cancelled() && !cancelled {
                cancelled = true;
                kill_tree(&mut child);
            }
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => thread::sleep(POLL),
                Err(problem) => {
                    return Err(RunError::Spawn {
                        program: invocation.program.clone(),
                        detail: problem.to_string(),
                    })
                }
            }
        };

        // A killed child can leave a grandchild holding the pipes open, and
        // `read_to_end` would then block for as long as that grandchild lives.
        // Cancelling must return promptly, so a cancelled run takes whatever
        // arrived and moves on.
        let (stdout, stderr) = if cancelled {
            (
                out_rx.recv_timeout(DRAIN_GRACE).unwrap_or_default(),
                err_rx.recv_timeout(DRAIN_GRACE).unwrap_or_default(),
            )
        } else {
            (
                out_rx.recv().unwrap_or_default(),
                err_rx.recv().unwrap_or_default(),
            )
        };
        Ok(Output {
            code: status.and_then(|status| status.code()),
            stdout: redact(&stdout),
            stderr: redact(&stderr),
            cancelled,
        })
    }
}

/// Whether the program can be run at all: a path as written, or a name on PATH.
fn program_exists(program: &str) -> bool {
    let has_separator = program.contains('/') || (cfg!(windows) && program.contains('\\'));
    if has_separator {
        return std::path::Path::new(program).is_file();
    }
    crate::detect::which(program).is_some()
}

/// Kill the child and anything it started.
///
/// `Child::kill` reaches only the process that was spawned. A shell that forked
/// leaves its children running, still holding the pipes, so the whole group has
/// to go.
#[cfg(unix)]
fn kill_tree(child: &mut std::process::Child) {
    // The child leads its own group, so the negated pid is that group.
    let pid = child.id() as i32;
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
    let _ = child.kill();
}

#[cfg(windows)]
fn kill_tree(child: &mut std::process::Child) {
    let _ = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &child.id().to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
}

/// Put the child at the head of its own process group so cancelling can take
/// the whole tree with it.
#[cfg(unix)]
fn own_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn own_process_group(command: &mut Command) {
    // CREATE_NEW_PROCESS_GROUP; taskkill /T walks the tree from the pid.
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0000_0200);
}

fn drain(pipe: Option<impl Read>) -> String {
    let mut buffer = Vec::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_end(&mut buffer);
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

/// A captured log is shown to the user, so a token that reaches a stream is
/// masked before it is ever stored.
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let _ = index;
        match line.find("Token:") {
            Some(at) => {
                out.push_str(&line[..at]);
                out.push_str("Token: ***");
                if line.ends_with('\n') {
                    out.push('\n');
                }
            }
            None => out.push_str(&mask_tokens(line)),
        }
    }
    out
}

const TOKEN_PREFIXES: [&str; 7] = [
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "gitlab-ci-token:",
];

fn mask_tokens(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let bytes: Vec<char> = line.chars().collect();
    let mut at = 0;
    'outer: while at < bytes.len() {
        let tail: String = bytes[at..].iter().collect();
        for prefix in TOKEN_PREFIXES {
            if let Some(rest) = tail.strip_prefix(prefix) {
                let body: String = rest
                    .chars()
                    .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                    .collect();
                if body.len() >= 16 {
                    out.push_str(prefix);
                    out.push_str("***");
                    at += prefix.chars().count() + body.chars().count();
                    continue 'outer;
                }
            }
        }
        out.push(bytes[at]);
        at += 1;
    }
    out
}
