//! Prepare GitHub Repo: an ordered list of named steps over git and gh. Strict
//! validation happens before the first step, and every failure carries a cause.

use crate::detect;
use crate::runner::{CancelToken, Invocation, Output, RunError, Runner, Sleeper, SystemSleeper};
use cartcore::cart::{cart_str, Cart};
use cartcore::findings::Finding;
use cartcore::labelart::label_art;
use cartcore::labeldoc::{serialize_doc, LabelDoc, DOC_FILE};
use cartcore::pack::bundle_name;
use cartcore::scaffold::{CHANGELOG_TEMPLATE, DEFAULT_LABEL, GITIGNORE_TEMPLATE, README_TEMPLATE};
use cartcore::schema::DEFAULT_SHELL;
use cartcore::workflow::{render, stamped_cart_id, WorkflowOptions, WORKFLOW_PATH};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepId {
    WriteFiles,
    WriteWorkflow,
    GitInit,
    GitAdd,
    GitCommit,
    RepoCreate,
    Tag,
    PushTag,
    WatchRun,
    ConfirmAsset,
}

/// The pipeline order, exactly as it runs.
pub const STEP_ORDER: [StepId; 10] = [
    StepId::WriteFiles,
    StepId::WriteWorkflow,
    StepId::GitInit,
    StepId::GitAdd,
    StepId::GitCommit,
    StepId::RepoCreate,
    StepId::Tag,
    StepId::PushTag,
    StepId::WatchRun,
    StepId::ConfirmAsset,
];

impl StepId {
    pub fn id(self) -> &'static str {
        match self {
            StepId::WriteFiles => "write_files",
            StepId::WriteWorkflow => "write_workflow",
            StepId::GitInit => "git_init",
            StepId::GitAdd => "git_add",
            StepId::GitCommit => "git_commit",
            StepId::RepoCreate => "repo_create",
            StepId::Tag => "tag",
            StepId::PushTag => "push_tag",
            StepId::WatchRun => "watch_run",
            StepId::ConfirmAsset => "confirm_asset",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            StepId::WriteFiles => "Write the cart directory",
            StepId::WriteWorkflow => "Write the release workflow",
            StepId::GitInit => "Initialise the repository",
            StepId::GitAdd => "Stage the cart",
            StepId::GitCommit => "Commit",
            StepId::RepoCreate => "Create the GitHub repository",
            StepId::Tag => "Tag the release",
            StepId::PushTag => "Push the tag",
            StepId::WatchRun => "Run the release workflow",
            StepId::ConfirmAsset => "Confirm the published bundle",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
}

/// One job step of the workflow run, surfaced while `watch_run` polls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunStage {
    pub job: String,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StepUpdate {
    pub step: StepId,
    pub id: &'static str,
    pub label: &'static str,
    pub state: StepState,
    pub detail: Option<String>,
    pub stages: Vec<RunStage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandLog {
    pub argv: Vec<String>,
    pub cwd: Option<String>,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StepLog {
    pub step: StepId,
    pub id: &'static str,
    pub state: StepState,
    pub detail: Option<String>,
    pub commands: Vec<CommandLog>,
}

/// Machine-readable failure, so the UI can offer the right fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    Validation,
    ToolMissing,
    NotAuthenticated,
    AuthExpired,
    RepoNameTaken,
    TagExists,
    ActionsDisabled,
    WorkflowFailed,
    RunMissing,
    AssetMissing,
    NoGitIdentity,
    DirtyTree,
    NothingToCommit,
    Network,
    Io,
    Cancelled,
    Unknown,
}

impl Cause {
    pub fn hint(self) -> &'static str {
        match self {
            Cause::Validation => "Fix the findings in the cart before publishing.",
            Cause::ToolMissing => "Install git and the GitHub CLI, then re-check.",
            Cause::NotAuthenticated => "Run `gh auth login` in a terminal, then re-check.",
            Cause::AuthExpired => "The GitHub credential was rejected; run `gh auth login` again.",
            Cause::RepoNameTaken => "That repository name is taken; pick another name.",
            Cause::TagExists => "That tag already exists; bump the cart version or delete the tag.",
            Cause::ActionsDisabled => "Enable GitHub Actions for the repository, then re-run.",
            Cause::WorkflowFailed => "Open the run log; the workflow re-validates with cartkit.",
            Cause::RunMissing => "No workflow run started for the tag; check Actions is enabled.",
            Cause::AssetMissing => "The workflow finished without attaching the bundle.",
            Cause::NoGitIdentity => "Set a name and email for this repository, then retry.",
            Cause::DirtyTree => "Commit or discard the remaining changes, then retry.",
            Cause::NothingToCommit => "There is nothing to commit in the cart directory.",
            Cause::Network => "GitHub was unreachable; check the connection and retry.",
            Cause::Io => "A file in the cart directory could not be written.",
            Cause::Cancelled => "Cancelled.",
            Cause::Unknown => "See the step log for the tool's own message.",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishError {
    pub step: Option<StepId>,
    pub step_id: Option<&'static str>,
    pub cause: Cause,
    pub message: String,
    pub stderr: String,
    pub hint: &'static str,
    pub findings: Vec<Finding>,
    pub steps: Vec<StepLog>,
}

impl std::fmt::Display for PublishError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.step_id {
            Some(step) => write!(out, "{}: {}", step, self.message),
            None => write!(out, "{}", self.message),
        }
    }
}

impl std::error::Error for PublishError {}

/// The error is boxed: it carries the whole step log with it.
pub type PublishResult<T> = Result<T, Box<PublishError>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PollOptions {
    pub interval: Duration,
    /// How many polls before the run is called missing or stuck.
    pub attempts: usize,
}

impl Default for PollOptions {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(5),
            attempts: 240,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GitIdentity {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone)]
pub struct PublishOptions {
    pub cart_dir: PathBuf,
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub private: bool,
    pub commit_message: String,
    /// Written to the new repository's LOCAL config when no identity is set.
    pub identity: Option<GitIdentity>,
    pub cartkit_repo: Option<String>,
    pub cartkit_ref: Option<String>,
    pub poll: PollOptions,
}

impl PublishOptions {
    pub fn new(
        cart_dir: impl Into<PathBuf>,
        owner: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            cart_dir: cart_dir.into(),
            owner: owner.into(),
            name: name.into(),
            description: None,
            private: false,
            commit_message: "Add cart".to_string(),
            identity: None,
            cartkit_repo: None,
            cartkit_ref: None,
            poll: PollOptions::default(),
        }
    }

    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublishOutcome {
    pub slug: String,
    pub repo_url: String,
    pub release_url: Option<String>,
    pub tag: String,
    pub asset: String,
    pub install_url: Option<String>,
    pub run_id: Option<i64>,
    pub private: bool,
    pub steps: Vec<StepLog>,
}

/// Strict validation, before anything is written or pushed.
pub fn preflight(cart_dir: &Path) -> PublishResult<Cart> {
    let cart = cartcore::read_cart(cart_dir).map_err(|problem| {
        Box::new(PublishError {
            step: None,
            step_id: None,
            cause: Cause::Validation,
            message: problem.to_string(),
            stderr: String::new(),
            hint: Cause::Validation.hint(),
            findings: Vec::new(),
            steps: Vec::new(),
        })
    })?;
    let report = cartcore::validate_cart(&cart, Some(cart_dir));
    if !report.ok(true) {
        return Err(Box::new(PublishError {
            step: None,
            step_id: None,
            cause: Cause::Validation,
            message: format!(
                "{} finding(s) must be cleared before publishing",
                report.findings.len()
            ),
            stderr: report
                .findings
                .iter()
                .map(Finding::line)
                .collect::<Vec<_>>()
                .join("\n"),
            hint: Cause::Validation.hint(),
            findings: report.findings,
            steps: Vec::new(),
        }));
    }
    Ok(cart)
}

pub fn publish(
    runner: &dyn Runner,
    cancel: &CancelToken,
    options: &PublishOptions,
    progress: &dyn Fn(&StepUpdate),
) -> PublishResult<PublishOutcome> {
    publish_with(runner, cancel, options, progress, &SystemSleeper)
}

pub fn publish_with(
    runner: &dyn Runner,
    cancel: &CancelToken,
    options: &PublishOptions,
    progress: &dyn Fn(&StepUpdate),
    sleeper: &dyn Sleeper,
) -> PublishResult<PublishOutcome> {
    let cart = preflight(&options.cart_dir)?;
    let mut session = Session {
        runner,
        cancel,
        progress,
        sleeper,
        steps: Vec::new(),
        current: None,
        commands: Vec::new(),
    };
    session.announce_pending();
    session.run(options, &cart)
}

struct Session<'a> {
    runner: &'a dyn Runner,
    cancel: &'a CancelToken,
    progress: &'a dyn Fn(&StepUpdate),
    sleeper: &'a dyn Sleeper,
    steps: Vec<StepLog>,
    current: Option<StepId>,
    commands: Vec<CommandLog>,
}

impl Session<'_> {
    fn announce_pending(&self) {
        for step in STEP_ORDER {
            self.emit(step, StepState::Pending, None, Vec::new());
        }
    }

    fn emit(&self, step: StepId, state: StepState, detail: Option<String>, stages: Vec<RunStage>) {
        (self.progress)(&StepUpdate {
            step,
            id: step.id(),
            label: step.label(),
            state,
            detail,
            stages,
        });
    }

    fn begin(&mut self, step: StepId) -> PublishResult<()> {
        self.current = Some(step);
        self.commands = Vec::new();
        self.emit(step, StepState::Running, None, Vec::new());
        self.guard()
    }

    fn finish(&mut self, step: StepId, state: StepState, detail: Option<String>) {
        self.steps.push(StepLog {
            step,
            id: step.id(),
            state,
            detail: detail.clone(),
            commands: std::mem::take(&mut self.commands),
        });
        self.emit(step, state, detail, Vec::new());
    }

    fn guard(&self) -> PublishResult<()> {
        if self.cancel.is_cancelled() {
            return Err(self.error(Cause::Cancelled, "cancelled", String::new()));
        }
        Ok(())
    }

    fn error(&self, cause: Cause, message: impl Into<String>, stderr: String) -> Box<PublishError> {
        let mut steps = self.steps.clone();
        if let Some(step) = self.current {
            steps.push(StepLog {
                step,
                id: step.id(),
                state: StepState::Failed,
                detail: None,
                commands: self.commands.clone(),
            });
        }
        Box::new(PublishError {
            step: self.current,
            step_id: self.current.map(StepId::id),
            cause,
            message: message.into(),
            stderr,
            hint: cause.hint(),
            findings: Vec::new(),
            steps,
        })
    }

    fn exec(&mut self, invocation: &Invocation) -> PublishResult<Output> {
        let output = match self.runner.run(invocation, self.cancel) {
            Ok(output) => output,
            Err(RunError::NotFound(program)) => {
                return Err(self.error(
                    Cause::ToolMissing,
                    format!("{} is not installed, or not on PATH", program),
                    String::new(),
                ))
            }
            Err(problem) => {
                return Err(self.error(Cause::Unknown, problem.to_string(), String::new()))
            }
        };
        self.commands.push(CommandLog {
            argv: invocation.argv(),
            cwd: invocation.cwd.as_ref().map(|dir| dir.display().to_string()),
            code: output.code,
            stdout: output.stdout.clone(),
            stderr: output.stderr.clone(),
        });
        if output.cancelled {
            return Err(self.error(Cause::Cancelled, "cancelled", String::new()));
        }
        Ok(output)
    }

    fn git(&mut self, dir: &Path, args: &[&str]) -> PublishResult<Output> {
        let invocation = Invocation::new(detect::GIT, args.to_vec()).in_dir(dir);
        self.exec(&invocation)
    }

    fn gh(&mut self, dir: &Path, args: &[String]) -> PublishResult<Output> {
        let invocation = Invocation::new(detect::GH, args.to_vec()).in_dir(dir);
        self.exec(&invocation)
    }

    fn io(&self, problem: std::io::Error, path: &Path) -> Box<PublishError> {
        self.error(
            Cause::Io,
            format!("{}: {}", path.display(), problem),
            String::new(),
        )
    }

    fn run(&mut self, options: &PublishOptions, cart: &Cart) -> PublishResult<PublishOutcome> {
        let dir = options.cart_dir.as_path();
        let cart_id = cart_str(cart, "id").unwrap_or_default().to_string();
        let version = cart_str(cart, "version").unwrap_or_default().to_string();
        let tag = format!("v{}", version);
        let asset = bundle_name(cart);
        let slug = options.slug();

        self.write_files(dir, cart)?;
        self.write_workflow(dir, options, &cart_id)?;
        self.git_init(dir, options)?;
        self.git_add(dir)?;
        self.git_commit(dir, options)?;
        self.repo_create(dir, options, &slug)?;
        self.tag(dir, &tag)?;
        self.push_tag(dir, &tag)?;
        let run_id = self.watch_run(dir, options, &slug, &tag)?;
        let release_url = self.confirm_asset(dir, &slug, &tag, &asset)?;

        Ok(PublishOutcome {
            repo_url: format!("https://github.com/{}", slug),
            install_url: release_url.as_ref().map(|_| {
                format!(
                    "https://github.com/{}/releases/download/{}/{}",
                    slug, tag, asset
                )
            }),
            slug,
            release_url,
            tag,
            asset,
            run_id,
            private: options.private,
            steps: std::mem::take(&mut self.steps),
        })
    }

    // ------- step 1: the cart directory

    fn write_files(&mut self, dir: &Path, cart: &Cart) -> PublishResult<()> {
        self.begin(StepId::WriteFiles)?;
        let label = cart_str(cart, "label").unwrap_or(DEFAULT_LABEL).to_string();
        let shell = cart_str(cart, "shell").unwrap_or(DEFAULT_SHELL).to_string();
        let mut written = Vec::new();

        if !cartcore::cart::cart_path(dir).is_file() {
            cartcore::cart::write_cart(dir, cart).map_err(|problem| self.io(problem, dir))?;
            written.push(cartcore::schema::CART_FILE.to_string());
        }
        for (name, body) in [
            ("README.md", substitute(README_TEMPLATE, cart)),
            ("CHANGELOG.md", substitute(CHANGELOG_TEMPLATE, cart)),
            (".gitignore", GITIGNORE_TEMPLATE.to_string()),
        ] {
            let path = dir.join(name);
            if !path.exists() {
                fs::write(&path, body).map_err(|problem| self.io(problem, &path))?;
                written.push(name.to_string());
            }
        }
        let label_path = dir.join(&label);
        if !label_path.exists() {
            if let Some(parent) = label_path.parent() {
                fs::create_dir_all(parent).map_err(|problem| self.io(problem, parent))?;
            }
            fs::write(&label_path, label_art(&shell))
                .map_err(|problem| self.io(problem, &label_path))?;
            written.push(label);
        }
        let doc_path = dir.join(DOC_FILE);
        if !doc_path.exists() {
            fs::write(&doc_path, serialize_doc(&LabelDoc::default()))
                .map_err(|problem| self.io(problem, &doc_path))?;
            written.push(DOC_FILE.to_string());
        }

        let detail = if written.is_empty() {
            "every file was already present".to_string()
        } else {
            format!("wrote {}", written.join(", "))
        };
        self.finish(StepId::WriteFiles, StepState::Done, Some(detail));
        Ok(())
    }

    // ------- step 2: the release workflow

    fn write_workflow(
        &mut self,
        dir: &Path,
        options: &PublishOptions,
        cart_id: &str,
    ) -> PublishResult<()> {
        self.begin(StepId::WriteWorkflow)?;
        let path = dir.join(WORKFLOW_PATH);
        let existing = fs::read_to_string(&path).ok();
        if let Some(body) = &existing {
            if stamped_cart_id(body).as_deref() == Some(cart_id) {
                self.finish(
                    StepId::WriteWorkflow,
                    StepState::Skipped,
                    Some(format!("{} already stamps {}", WORKFLOW_PATH, cart_id)),
                );
                return Ok(());
            }
        }
        let mut workflow = WorkflowOptions::new(cart_id);
        if let Some(repo) = &options.cartkit_repo {
            workflow.cartkit_repo = repo.clone();
        }
        if let Some(reference) = &options.cartkit_ref {
            workflow.cartkit_ref = reference.clone();
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|problem| self.io(problem, parent))?;
        }
        fs::write(&path, render(&workflow)).map_err(|problem| self.io(problem, &path))?;
        self.finish(
            StepId::WriteWorkflow,
            StepState::Done,
            Some(format!("wrote {}", WORKFLOW_PATH)),
        );
        Ok(())
    }

    // ------- step 3: init, plus the identity a commit needs

    fn git_init(&mut self, dir: &Path, options: &PublishOptions) -> PublishResult<()> {
        self.begin(StepId::GitInit)?;
        let already = dir.join(".git").exists();
        if !already {
            let output = self.git(dir, &["init", "-b", "main"])?;
            if !output.success() {
                let problem = output.problem();
                return Err(self.error(classify(&problem), "git init failed", problem));
            }
        }
        let identity = detect::dir_identity(self.runner, self.cancel, dir);
        if !identity.complete() {
            match &options.identity {
                Some(wanted) => {
                    detect::set_identity(
                        self.runner,
                        self.cancel,
                        dir,
                        &wanted.name,
                        &wanted.email,
                    )
                    .map_err(|problem| {
                        self.error(Cause::NoGitIdentity, "git identity", problem.to_string())
                    })?;
                }
                None => {
                    return Err(self.error(
                        Cause::NoGitIdentity,
                        "git has no user.name or user.email for this repository",
                        String::new(),
                    ))
                }
            }
        }
        let state = if already {
            StepState::Skipped
        } else {
            StepState::Done
        };
        let detail = already.then(|| "an existing repository was reused".to_string());
        self.finish(StepId::GitInit, state, detail);
        Ok(())
    }

    fn git_add(&mut self, dir: &Path) -> PublishResult<()> {
        self.begin(StepId::GitAdd)?;
        let output = self.git(dir, &["add", "-A"])?;
        if !output.success() {
            let problem = output.problem();
            return Err(self.error(classify(&problem), "git add failed", problem));
        }
        self.finish(StepId::GitAdd, StepState::Done, None);
        Ok(())
    }

    fn git_commit(&mut self, dir: &Path, options: &PublishOptions) -> PublishResult<()> {
        self.begin(StepId::GitCommit)?;
        let status = self.git(dir, &["status", "--porcelain"])?;
        if status.stdout.trim().is_empty() {
            let head = self.git(dir, &["rev-parse", "--verify", "HEAD"])?;
            if head.success() {
                self.finish(
                    StepId::GitCommit,
                    StepState::Skipped,
                    Some("the tree already matches the last commit".to_string()),
                );
                return Ok(());
            }
            return Err(self.error(
                Cause::NothingToCommit,
                "there is nothing to commit in the cart directory",
                String::new(),
            ));
        }
        let output = self.git(dir, &["commit", "-m", &options.commit_message])?;
        if !output.success() {
            let problem = output.problem();
            return Err(self.error(classify(&problem), "git commit failed", problem));
        }
        self.finish(StepId::GitCommit, StepState::Done, None);
        Ok(())
    }

    // ------- step 4: the repository itself

    fn repo_create(
        &mut self,
        dir: &Path,
        options: &PublishOptions,
        slug: &str,
    ) -> PublishResult<()> {
        self.begin(StepId::RepoCreate)?;
        let origin = self.git(dir, &["remote", "get-url", "origin"])?;
        if origin.success() && !origin.stdout.trim().is_empty() {
            let pushed = self.git(dir, &["push", "-u", "origin", "main"])?;
            if !pushed.success() {
                let problem = pushed.problem();
                return Err(self.error(classify(&problem), "git push failed", problem));
            }
            self.finish(
                StepId::RepoCreate,
                StepState::Skipped,
                Some(format!("origin already points at {}", origin.stdout.trim())),
            );
            return Ok(());
        }

        let mut args = vec![
            "repo".to_string(),
            "create".to_string(),
            slug.to_string(),
            if options.private {
                "--private".to_string()
            } else {
                "--public".to_string()
            },
        ];
        if let Some(description) = &options.description {
            args.push("--description".to_string());
            args.push(description.clone());
        }
        args.extend(
            ["--source", ".", "--remote", "origin", "--push"]
                .iter()
                .map(|arg| arg.to_string()),
        );
        let output = self.gh(dir, &args)?;
        if !output.success() {
            let problem = output.problem();
            return Err(self.error(classify(&problem), "gh repo create failed", problem));
        }
        self.finish(
            StepId::RepoCreate,
            StepState::Done,
            Some(format!("created {}", slug)),
        );
        Ok(())
    }

    // ------- step 5: the tag

    fn tag(&mut self, dir: &Path, tag: &str) -> PublishResult<()> {
        self.begin(StepId::Tag)?;
        let status = self.git(dir, &["status", "--porcelain"])?;
        if !status.stdout.trim().is_empty() {
            return Err(self.error(
                Cause::DirtyTree,
                "the cart directory has uncommitted changes; the tag would not describe it",
                status.stdout.trim().to_string(),
            ));
        }
        let listed = self.git(dir, &["tag", "--list", tag])?;
        if listed.success() && !listed.stdout.trim().is_empty() {
            let at_tag = self.git(dir, &["rev-list", "-n", "1", tag])?;
            let at_head = self.git(dir, &["rev-parse", "HEAD"])?;
            if at_tag.stdout.trim() == at_head.stdout.trim() && at_tag.success() {
                self.finish(
                    StepId::Tag,
                    StepState::Skipped,
                    Some(format!("{} already points at this commit", tag)),
                );
                return Ok(());
            }
            return Err(self.error(
                Cause::TagExists,
                format!("{} already exists and points at another commit", tag),
                String::new(),
            ));
        }
        let output = self.git(dir, &["tag", tag])?;
        if !output.success() {
            let problem = output.problem();
            return Err(self.error(classify(&problem), "git tag failed", problem));
        }
        self.finish(StepId::Tag, StepState::Done, Some(tag.to_string()));
        Ok(())
    }

    fn push_tag(&mut self, dir: &Path, tag: &str) -> PublishResult<()> {
        self.begin(StepId::PushTag)?;
        let output = self.git(dir, &["push", "origin", tag])?;
        if !output.success() {
            let problem = output.problem();
            let cause = if problem.contains("already exists") || problem.contains("rejected") {
                Cause::TagExists
            } else {
                classify(&problem)
            };
            return Err(self.error(cause, "git push of the tag failed", problem));
        }
        self.finish(StepId::PushTag, StepState::Done, Some(tag.to_string()));
        Ok(())
    }

    // ------- step 6: the workflow run

    fn watch_run(
        &mut self,
        dir: &Path,
        options: &PublishOptions,
        slug: &str,
        tag: &str,
    ) -> PublishResult<Option<i64>> {
        self.begin(StepId::WatchRun)?;
        let list = vec![
            "run".to_string(),
            "list".to_string(),
            "--repo".to_string(),
            slug.to_string(),
            "--branch".to_string(),
            tag.to_string(),
            "--json".to_string(),
            "databaseId,status,conclusion,headBranch,event,name".to_string(),
            "--limit".to_string(),
            "20".to_string(),
        ];

        let mut run_id = None;
        for attempt in 0..options.poll.attempts.max(1) {
            self.guard()?;
            let output = self.gh(dir, &list)?;
            if !output.success() {
                let problem = output.problem();
                return Err(self.error(classify(&problem), "gh run list failed", problem));
            }
            if let Some(found) = pick_run(&output.stdout, tag) {
                run_id = Some(found);
                break;
            }
            if attempt + 1 < options.poll.attempts.max(1) {
                self.sleeper.sleep(options.poll.interval);
            }
        }
        let Some(run_id) = run_id else {
            return Err(self.error(
                Cause::RunMissing,
                format!("no workflow run appeared for {}", tag),
                String::new(),
            ));
        };

        let view = vec![
            "run".to_string(),
            "view".to_string(),
            run_id.to_string(),
            "--repo".to_string(),
            slug.to_string(),
            "--json".to_string(),
            "status,conclusion,jobs,url".to_string(),
        ];
        for attempt in 0..options.poll.attempts.max(1) {
            self.guard()?;
            let output = self.gh(dir, &view)?;
            if !output.success() {
                let problem = output.problem();
                return Err(self.error(classify(&problem), "gh run view failed", problem));
            }
            let doc: Value = serde_json::from_str(&output.stdout).unwrap_or(Value::Null);
            let stages = stages_of(&doc);
            let status = doc
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            self.emit(
                StepId::WatchRun,
                StepState::Running,
                Some(status.clone()),
                stages.clone(),
            );
            if status == "completed" {
                let conclusion = doc
                    .get("conclusion")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if conclusion == "success" {
                    self.finish(
                        StepId::WatchRun,
                        StepState::Done,
                        Some(format!("run {} succeeded", run_id)),
                    );
                    return Ok(Some(run_id));
                }
                let failed = stages
                    .iter()
                    .find(|stage| {
                        matches!(
                            stage.conclusion.as_deref(),
                            Some("failure") | Some("cancelled")
                        )
                    })
                    .map(|stage| format!("{} / {}", stage.job, stage.name))
                    .unwrap_or_else(|| format!("run {}", run_id));
                return Err(self.error(
                    Cause::WorkflowFailed,
                    format!("the release workflow {} at {}", conclusion, failed),
                    output.stdout.clone(),
                ));
            }
            if attempt + 1 < options.poll.attempts.max(1) {
                self.sleeper.sleep(options.poll.interval);
            }
        }
        Err(self.error(
            Cause::WorkflowFailed,
            format!("run {} did not finish in time", run_id),
            String::new(),
        ))
    }

    // ------- step 7: the asset

    fn confirm_asset(
        &mut self,
        dir: &Path,
        slug: &str,
        tag: &str,
        asset: &str,
    ) -> PublishResult<Option<String>> {
        self.begin(StepId::ConfirmAsset)?;
        let args = vec![
            "release".to_string(),
            "view".to_string(),
            tag.to_string(),
            "--repo".to_string(),
            slug.to_string(),
            "--json".to_string(),
            "tagName,url,assets".to_string(),
        ];
        let output = self.gh(dir, &args)?;
        if !output.success() {
            let problem = output.problem();
            return Err(self.error(classify(&problem), "gh release view failed", problem));
        }
        let doc: Value = serde_json::from_str(&output.stdout).unwrap_or(Value::Null);
        let names = asset_names(&doc);
        if !names.iter().any(|name| name == asset) {
            return Err(self.error(
                Cause::AssetMissing,
                format!("{} is not attached to {}", asset, tag),
                if names.is_empty() {
                    "the release has no assets".to_string()
                } else {
                    format!("attached: {}", names.join(", "))
                },
            ));
        }
        let url = doc
            .get("url")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("https://github.com/{}/releases/tag/{}", slug, tag));
        self.finish(
            StepId::ConfirmAsset,
            StepState::Done,
            Some(format!("{} is attached to {}", asset, tag)),
        );
        Ok(Some(url))
    }
}

fn substitute(template: &str, cart: &Cart) -> String {
    let get = |key: &str| cart_str(cart, key).unwrap_or_default().to_string();
    template
        .replace("{{id}}", &get("id"))
        .replace("{{title}}", &get("title"))
        .replace("{{base}}", &get("base"))
        .replace("{{seal}}", &get("seal"))
        .replace("{{label}}", &get("label"))
}

/// The newest run for the tag; gh reports a tag push with the tag as headBranch.
pub fn pick_run(json: &str, tag: &str) -> Option<i64> {
    let doc: Value = serde_json::from_str(json).ok()?;
    let runs = doc.as_array()?;
    let matching = runs
        .iter()
        .find(|run| run.get("headBranch").and_then(Value::as_str) == Some(tag))
        .or_else(|| runs.first())?;
    matching.get("databaseId").and_then(Value::as_i64)
}

pub fn stages_of(doc: &Value) -> Vec<RunStage> {
    let mut stages = Vec::new();
    let Some(jobs) = doc.get("jobs").and_then(Value::as_array) else {
        return stages;
    };
    for job in jobs {
        let job_name = job
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("job")
            .to_string();
        match job.get("steps").and_then(Value::as_array) {
            Some(steps) => {
                for step in steps {
                    stages.push(RunStage {
                        job: job_name.clone(),
                        name: step
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        status: step
                            .get("status")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        conclusion: step
                            .get("conclusion")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    });
                }
            }
            None => stages.push(RunStage {
                name: job_name.clone(),
                job: job_name,
                status: job
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                conclusion: job
                    .get("conclusion")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            }),
        }
    }
    stages
}

pub fn asset_names(doc: &Value) -> Vec<String> {
    doc.get("assets")
        .and_then(Value::as_array)
        .map(|assets| {
            assets
                .iter()
                .filter_map(|asset| asset.get("name").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// What git and gh say when they fail, mapped onto a cause the UI can act on.
pub fn classify(problem: &str) -> Cause {
    let text = problem.to_ascii_lowercase();
    let has = |needle: &str| text.contains(needle);

    if has("could not resolve host")
        || has("network is unreachable")
        || has("connection reset")
        || has("connection refused")
        || has("timed out")
        || has("temporary failure in name resolution")
        || has("dial tcp")
    {
        return Cause::Network;
    }
    if has("gh auth login") || has("not logged into") || has("no github credential") {
        return Cause::NotAuthenticated;
    }
    if has("actions") && (has("disabled") || has("not enabled")) {
        return Cause::ActionsDisabled;
    }
    if has("bad credentials")
        || has("http 401")
        || has("authentication failed")
        || has("token expired")
        || has("requires authentication")
        || has("http 403")
    {
        return Cause::AuthExpired;
    }
    if has("name already exists") || has("repository already exists") {
        return Cause::RepoNameTaken;
    }
    if has("tag") && has("already exists") {
        return Cause::TagExists;
    }
    if has("please tell me who you are")
        || has("empty ident name")
        || has("unable to auto-detect email address")
    {
        return Cause::NoGitIdentity;
    }
    if has("nothing to commit") {
        return Cause::NothingToCommit;
    }
    Cause::Unknown
}
