//! The window's view of library types. The contract lives in src/lib/types.ts;
//! nothing here adds behavior, it only shapes what crosses the boundary.

use serde::Serialize;
use toolchain::detect::{AuthStatus, Credential, Identity, ToolStatus};
use toolchain::instructions::InstallGuide;
use toolchain::publish::CommandLog;
use toolchain::readiness::Readiness;
use toolchain::submit::{SubmissionKind, SubmissionPlan};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub found: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub detail: Option<String>,
}

impl From<&ToolStatus> for Tool {
    fn from(status: &ToolStatus) -> Self {
        Self {
            found: status.found,
            version: status.version.clone(),
            path: status.path.clone(),
            detail: status.detail.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhTool {
    pub found: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub detail: Option<String>,
    pub authenticated: bool,
    pub account: Option<String>,
    pub protocol: Option<String>,
    pub scopes: Vec<String>,
    /// The variable gh will read a token from, when there is one.
    pub token_env: Option<String>,
    pub credential_note: String,
}

impl GhTool {
    pub fn new(status: &ToolStatus, auth: &AuthStatus) -> Self {
        Self {
            found: status.found,
            version: status.version.clone(),
            path: status.path.clone(),
            detail: status.detail.clone(),
            authenticated: auth.authenticated,
            account: auth.account.clone(),
            protocol: auth.protocol.clone(),
            scopes: auth.scopes.clone(),
            token_env: match &auth.credential {
                Credential::Environment { variable } => Some(variable.clone()),
                _ => None,
            },
            credential_note: auth.credential_note.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitIdentity {
    pub name: Option<String>,
    pub email: Option<String>,
}

impl From<&Identity> for GitIdentity {
    fn from(identity: &Identity) -> Self {
        Self {
            name: identity.name.clone(),
            email: identity.email.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallStep {
    pub label: String,
    pub command: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallInstructions {
    pub tool: String,
    pub os: String,
    pub title: String,
    pub note: Option<String>,
    pub notes: Vec<String>,
    pub steps: Vec<InstallStep>,
}

impl From<&InstallGuide> for InstallInstructions {
    fn from(guide: &InstallGuide) -> Self {
        Self {
            tool: guide.tool.program().to_string(),
            os: format!("{:?}", guide.platform).to_lowercase(),
            title: format!("Install {}", guide.tool.program()),
            note: guide.notes.first().cloned(),
            notes: guide.notes.clone(),
            steps: guide
                .options
                .iter()
                .map(|option| InstallStep {
                    label: option.label.clone(),
                    command: option.display_command(),
                    url: option.url.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessItem {
    pub id: String,
    pub label: String,
    pub ok: bool,
    pub blocking: bool,
    pub detail: String,
    pub fix: Option<String>,
    /// Which action the window should offer; the label alone is not enough to
    /// wire a button to.
    pub fix_id: Option<String>,
    pub fix_command: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessReport {
    pub items: Vec<ReadinessItem>,
    pub ready: bool,
    pub unknown: Vec<String>,
}

impl From<&Readiness> for ReadinessReport {
    fn from(readiness: &Readiness) -> Self {
        Self {
            items: readiness
                .items
                .iter()
                .map(|item| ReadinessItem {
                    id: item.id.clone(),
                    label: item.label.clone(),
                    ok: item.ok,
                    blocking: item.blocking,
                    detail: item.detail.clone(),
                    fix: item.fix.as_ref().map(|fix| fix.label.clone()),
                    fix_id: item.fix.as_ref().map(|fix| fix.id.clone()),
                    fix_command: item.fix.as_ref().and_then(|fix| fix.command.clone()),
                })
                .collect(),
            ready: readiness.ready,
            unknown: readiness.unknown.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAsset {
    pub name: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub tag: String,
    pub name: Option<String>,
    pub published_at: Option<String>,
    pub prerelease: bool,
    pub assets: Vec<ReleaseAsset>,
}

impl From<&resolve::github::ReleaseSummary> for Release {
    fn from(release: &resolve::github::ReleaseSummary) -> Self {
        Self {
            tag: release.tag.clone(),
            name: release.name.clone(),
            published_at: release.published_at.clone(),
            prerelease: release.prerelease,
            assets: release
                .assets
                .iter()
                .map(|asset| ReleaseAsset {
                    name: asset.name.clone(),
                    size: asset.size,
                    url: asset.url.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameBananaFile {
    pub id: Option<u64>,
    pub file: String,
    pub size: u64,
    pub md5: String,
    pub description: String,
    pub downloads: u64,
}

impl From<&resolve::gamebanana::GbFile> for GameBananaFile {
    fn from(file: &resolve::gamebanana::GbFile) -> Self {
        Self {
            id: file.id,
            file: file.file.clone(),
            size: file.filesize,
            md5: file.md5.clone(),
            description: file.description.clone(),
            downloads: file.download_count,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionField {
    pub id: String,
    pub label: String,
    pub value: String,
    pub multiline: bool,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionPlanView {
    pub kind: String,
    pub repo: String,
    pub url: String,
    pub title: String,
    pub body: String,
    pub fields: Vec<SubmissionField>,
    pub labels: Vec<String>,
    pub data_file: Option<String>,
    pub guidance: String,
}

impl From<&SubmissionPlan> for SubmissionPlanView {
    fn from(plan: &SubmissionPlan) -> Self {
        let kind = match plan.kind {
            SubmissionKind::Issue | SubmissionKind::IssueForm => "issue",
            SubmissionKind::PullRequest => "pull_request",
        };
        let url = match plan.kind {
            SubmissionKind::PullRequest => format!("https://github.com/{}", plan.repo),
            _ => format!("https://github.com/{}/issues/new", plan.repo),
        };
        Self {
            kind: kind.to_string(),
            repo: plan.repo.clone(),
            url,
            title: plan.title.clone(),
            body: plan.body.clone(),
            fields: plan
                .fields
                .iter()
                .map(|field| SubmissionField {
                    id: field.id.clone(),
                    label: field.label.clone(),
                    value: field.value.clone(),
                    multiline: field.value.contains('\n') || field.value.len() > 80,
                    required: field.required,
                })
                .collect(),
            labels: plan.labels.clone(),
            data_file: plan.data_file.clone(),
            guidance: plan.guidance.join(" "),
        }
    }
}

/// One expandable log per step: the argv, then whatever the tool said.
pub fn render_log(commands: &[CommandLog]) -> String {
    let mut out = String::new();
    for command in commands {
        out.push_str(&format!("$ {}\n", command.argv.join(" ")));
        if !command.stdout.trim().is_empty() {
            out.push_str(command.stdout.trim_end());
            out.push('\n');
        }
        if !command.stderr.trim().is_empty() {
            out.push_str(command.stderr.trim_end());
            out.push('\n');
        }
    }
    out
}
