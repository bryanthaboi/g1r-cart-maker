//! The release workflow a generated cart repo ships. CI re-validates with the
//! authoritative cartkit, so the template stamps CART_ID and nothing else.

pub const TEMPLATE: &str = include_str!("../templates/release.yml");
pub const WORKFLOW_PATH: &str = ".github/workflows/release.yml";
pub const DEFAULT_CARTKIT_REPO: &str = "bryanthaboi/gen1recomp";
pub const DEFAULT_CARTKIT_REF: &str = "dev";

#[derive(Debug, Clone)]
pub struct WorkflowOptions {
    pub cart_id: String,
    pub cartkit_repo: String,
    pub cartkit_ref: String,
}

impl WorkflowOptions {
    pub fn new(cart_id: impl Into<String>) -> Self {
        Self {
            cart_id: cart_id.into(),
            cartkit_repo: DEFAULT_CARTKIT_REPO.to_string(),
            cartkit_ref: DEFAULT_CARTKIT_REF.to_string(),
        }
    }
}

/// Render the workflow. An author can retarget cartkit by editing the env block.
pub fn render(options: &WorkflowOptions) -> String {
    let body = TEMPLATE.replace("{{CART_ID}}", &options.cart_id);
    let body = body.replace(
        &format!("CARTKIT_REPO: {}", DEFAULT_CARTKIT_REPO),
        &format!("CARTKIT_REPO: {}", options.cartkit_repo),
    );
    body.replace(
        &format!("CARTKIT_REF: {}", DEFAULT_CARTKIT_REF),
        &format!("CARTKIT_REF: {}", options.cartkit_ref),
    )
}

/// cartkit stamps the id into the workflow; a rename has to rewrite it.
pub fn stamped_cart_id(body: &str) -> Option<String> {
    for line in body.lines() {
        if let Some(rest) = line.trim().strip_prefix("CART_ID:") {
            return Some(rest.trim().trim_matches('"').to_string());
        }
    }
    None
}
