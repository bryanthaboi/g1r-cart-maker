//! Per-OS install instructions for git and gh, as structured rows. This crate
//! never runs a package manager and never runs anything with sudo.

use serde::Serialize;
use std::fs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    Git,
    Gh,
}

impl Tool {
    pub fn program(self) -> &'static str {
        match self {
            Tool::Git => "git",
            Tool::Gh => "gh",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinuxFamily {
    Debian,
    Fedora,
    Arch,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "os", rename_all = "snake_case")]
pub enum Platform {
    MacOs,
    Windows,
    Linux { family: LinuxFamily },
    Other,
}

/// The command is an argument array; the string form is for display and the
/// clipboard only, never for execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallOption {
    pub label: String,
    pub command: Option<Vec<String>>,
    pub url: Option<String>,
}

impl InstallOption {
    fn command(label: &str, command: &[&str]) -> Self {
        Self {
            label: label.to_string(),
            command: Some(command.iter().map(|part| part.to_string()).collect()),
            url: None,
        }
    }

    fn link(label: &str, url: &str) -> Self {
        Self {
            label: label.to_string(),
            command: None,
            url: Some(url.to_string()),
        }
    }

    pub fn display_command(&self) -> Option<String> {
        self.command.as_ref().map(|parts| parts.join(" "))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallGuide {
    pub tool: Tool,
    pub platform: Platform,
    pub options: Vec<InstallOption>,
    pub notes: Vec<String>,
}

const PATH_NOTE: &str = "After installing, open a new terminal or restart this app before re-checking; PATH is read at process start.";
const RECHECK_NOTE: &str = "Run the install yourself, then use Re-check.";

pub fn detect_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::MacOs
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "linux") {
        Platform::Linux {
            family: detect_linux_family(),
        }
    } else {
        Platform::Other
    }
}

pub fn detect_linux_family() -> LinuxFamily {
    let release = fs::read_to_string("/etc/os-release").unwrap_or_default();
    linux_family_from_os_release(&release)
}

pub fn linux_family_from_os_release(release: &str) -> LinuxFamily {
    let mut ids = Vec::new();
    for line in release.lines() {
        let line = line.trim();
        if let Some(rest) = line
            .strip_prefix("ID=")
            .or_else(|| line.strip_prefix("ID_LIKE="))
        {
            for id in rest.trim_matches('"').split_whitespace() {
                ids.push(id.to_ascii_lowercase());
            }
        }
    }
    let has = |name: &str| ids.iter().any(|id| id == name);
    if has("debian") || has("ubuntu") {
        LinuxFamily::Debian
    } else if has("fedora") || has("rhel") || has("centos") {
        LinuxFamily::Fedora
    } else if has("arch") || has("archlinux") || has("manjaro") {
        LinuxFamily::Arch
    } else {
        LinuxFamily::Other
    }
}

pub fn guide(tool: Tool, platform: Platform) -> InstallGuide {
    let (options, notes) = match (tool, platform) {
        (Tool::Git, Platform::MacOs) => (
            vec![
                InstallOption::command("Apple command line tools", &["xcode-select", "--install"]),
                InstallOption::command("Homebrew", &["brew", "install", "git"]),
                InstallOption::link("No Homebrew? Install it", "https://brew.sh"),
                InstallOption::link("Official macOS installer", "https://git-scm.com/download/mac"),
            ],
            vec![RECHECK_NOTE.to_string()],
        ),
        (Tool::Gh, Platform::MacOs) => (
            vec![
                InstallOption::command("Homebrew", &["brew", "install", "gh"]),
                InstallOption::link("No Homebrew? Install it", "https://brew.sh"),
                InstallOption::link(
                    "Official .pkg installer",
                    "https://github.com/cli/cli/releases/latest",
                ),
            ],
            vec![RECHECK_NOTE.to_string()],
        ),
        (Tool::Git, Platform::Windows) => (
            vec![
                InstallOption::command(
                    "winget",
                    &["winget", "install", "--id", "Git.Git", "-e"],
                ),
                InstallOption::command("Scoop", &["scoop", "install", "git"]),
                InstallOption::link("Official installer", "https://git-scm.com/download/win"),
            ],
            vec![PATH_NOTE.to_string(), RECHECK_NOTE.to_string()],
        ),
        (Tool::Gh, Platform::Windows) => (
            vec![
                InstallOption::command(
                    "winget",
                    &["winget", "install", "--id", "GitHub.cli", "-e"],
                ),
                InstallOption::command("Scoop", &["scoop", "install", "gh"]),
                InstallOption::link(
                    "Official installer",
                    "https://github.com/cli/cli/releases/latest",
                ),
            ],
            vec![PATH_NOTE.to_string(), RECHECK_NOTE.to_string()],
        ),
        (Tool::Git, Platform::Linux { family }) => (
            linux_git(family),
            vec![RECHECK_NOTE.to_string()],
        ),
        (Tool::Gh, Platform::Linux { family }) => (
            linux_gh(family),
            vec![
                "Distro packages of gh are often stale; the cli.github.com apt repository is the current one.".to_string(),
                RECHECK_NOTE.to_string(),
            ],
        ),
        (Tool::Git, Platform::Other) => (
            vec![InstallOption::link(
                "Downloads for every platform",
                "https://git-scm.com/downloads",
            )],
            vec![RECHECK_NOTE.to_string()],
        ),
        (Tool::Gh, Platform::Other) => (
            vec![InstallOption::link(
                "Release tarballs and the AppImage",
                "https://github.com/cli/cli/releases/latest",
            )],
            vec![RECHECK_NOTE.to_string()],
        ),
    };
    InstallGuide {
        tool,
        platform,
        options,
        notes,
    }
}

fn linux_git(family: LinuxFamily) -> Vec<InstallOption> {
    let mut options = match family {
        LinuxFamily::Debian => vec![InstallOption::command(
            "Debian / Ubuntu",
            &["sudo", "apt", "install", "git"],
        )],
        LinuxFamily::Fedora => vec![InstallOption::command(
            "Fedora",
            &["sudo", "dnf", "install", "git"],
        )],
        LinuxFamily::Arch => vec![InstallOption::command(
            "Arch",
            &["sudo", "pacman", "-S", "git"],
        )],
        LinuxFamily::Other => vec![
            InstallOption::command("Debian / Ubuntu", &["sudo", "apt", "install", "git"]),
            InstallOption::command("Fedora", &["sudo", "dnf", "install", "git"]),
            InstallOption::command("Arch", &["sudo", "pacman", "-S", "git"]),
        ],
    };
    options.push(InstallOption::link(
        "Other distributions",
        "https://git-scm.com/download/linux",
    ));
    options
}

fn linux_gh(family: LinuxFamily) -> Vec<InstallOption> {
    let apt_repo = InstallOption::link(
        "Debian / Ubuntu: the official apt repository",
        "https://github.com/cli/cli/blob/trunk/docs/install_linux.md",
    );
    let mut options = match family {
        LinuxFamily::Debian => vec![
            apt_repo,
            InstallOption::command(
                "Debian / Ubuntu, once the repository is added",
                &["sudo", "apt", "install", "gh"],
            ),
        ],
        LinuxFamily::Fedora => vec![InstallOption::command(
            "Fedora",
            &["sudo", "dnf", "install", "gh"],
        )],
        LinuxFamily::Arch => vec![InstallOption::command(
            "Arch",
            &["sudo", "pacman", "-S", "github-cli"],
        )],
        LinuxFamily::Other => vec![
            apt_repo,
            InstallOption::command("Fedora", &["sudo", "dnf", "install", "gh"]),
            InstallOption::command("Arch", &["sudo", "pacman", "-S", "github-cli"]),
        ],
    };
    options.push(InstallOption::link(
        "Release tarball or AppImage",
        "https://github.com/cli/cli/releases/latest",
    ));
    options
}

/// Both tools for one platform, which is what the environment panel shows.
pub fn guides(platform: Platform) -> Vec<InstallGuide> {
    vec![guide(Tool::Git, platform), guide(Tool::Gh, platform)]
}

/// gh owns the credential; the app only tells the user to run this.
pub fn auth_login_hint() -> InstallOption {
    InstallOption::command("Sign in to GitHub", &["gh", "auth", "login"])
}
