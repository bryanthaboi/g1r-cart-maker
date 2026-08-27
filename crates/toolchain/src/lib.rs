//! git and gh orchestration: environment detection, the Prepare GitHub Repo
//! pipeline, the index-readiness checklist and index submission. Every
//! subprocess is an argument array; nothing here builds a shell string.

pub mod detect;
pub mod fake;
pub mod instructions;
pub mod publish;
pub mod readiness;
pub mod runner;
pub mod submit;

pub use detect::{detect, AuthStatus, Credential, Identity, TokenEnv, ToolStatus, Toolchain};
pub use publish::{
    preflight, publish, publish_with, Cause, PublishError, PublishOptions, PublishOutcome,
    PublishResult, StepId, StepLog, StepState, StepUpdate, STEP_ORDER,
};
pub use readiness::{IndexHints, Readiness, ReadinessItem, RemoteFacts};
pub use runner::{CancelToken, Invocation, Output, RunError, Runner, SystemRunner};
pub use submit::{Discovery, Submission, SubmissionKind, SubmissionPlan, INDEX_REPO};
