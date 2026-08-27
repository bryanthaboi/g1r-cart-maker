//! cartkit's three-way findings model: an error fails, a warning fails only
//! under strict (and pack is always strict), a note never fails.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warn,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warn => "warn",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    pub path: Option<String>,
}

impl Finding {
    pub fn line(&self) -> String {
        let where_ = match &self.path {
            Some(path) => format!("{}: ", path),
            None => String::new(),
        };
        format!(
            "{} {:5} {}{}",
            self.rule,
            self.severity.as_str().to_uppercase(),
            where_,
            self.message
        )
    }
}

pub fn err(rule: &str, message: impl Into<String>) -> Finding {
    Finding {
        rule: rule.to_string(),
        severity: Severity::Error,
        message: message.into(),
        path: Some(crate::schema::CART_FILE.to_string()),
    }
}

pub fn warn(rule: &str, message: impl Into<String>) -> Finding {
    Finding {
        rule: rule.to_string(),
        severity: Severity::Warn,
        message: message.into(),
        path: Some(crate::schema::CART_FILE.to_string()),
    }
}

pub fn err_at(rule: &str, message: impl Into<String>, path: &str) -> Finding {
    Finding {
        path: Some(path.to_string()),
        ..err(rule, message)
    }
}

pub fn warn_at(rule: &str, message: impl Into<String>, path: &str) -> Finding {
    Finding {
        path: Some(path.to_string()),
        ..warn(rule, message)
    }
}

/// The full outcome of a validation pass. Notes are advisory only.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub notes: Vec<String>,
}

impl Report {
    pub fn errors(&self) -> impl Iterator<Item = &Finding> {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Finding> {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Warn)
    }

    /// cartkit's exit condition: strict counts warnings as failures.
    pub fn ok(&self, strict: bool) -> bool {
        if strict {
            self.findings.is_empty()
        } else {
            self.errors().next().is_none()
        }
    }
}
