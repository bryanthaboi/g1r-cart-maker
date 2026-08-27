use toolchain::instructions::{
    auth_login_hint, guide, guides, linux_family_from_os_release, LinuxFamily, Platform, Tool,
};

fn commands(tool: Tool, platform: Platform) -> Vec<Vec<String>> {
    guide(tool, platform)
        .options
        .into_iter()
        .filter_map(|option| option.command)
        .collect()
}

fn urls(tool: Tool, platform: Platform) -> Vec<String> {
    guide(tool, platform)
        .options
        .into_iter()
        .filter_map(|option| option.url)
        .collect()
}

#[test]
fn macos_offers_xcode_select_homebrew_and_the_installers() {
    assert_eq!(
        commands(Tool::Git, Platform::MacOs),
        vec![
            vec!["xcode-select", "--install"],
            vec!["brew", "install", "git"]
        ]
    );
    assert!(urls(Tool::Git, Platform::MacOs).contains(&"https://brew.sh".to_string()));
    assert_eq!(
        commands(Tool::Gh, Platform::MacOs),
        vec![vec!["brew", "install", "gh"]]
    );
    assert!(urls(Tool::Gh, Platform::MacOs)
        .iter()
        .any(|url| url.contains("cli/cli/releases")));
}

#[test]
fn windows_offers_the_winget_ids_scoop_and_the_path_note() {
    assert_eq!(
        commands(Tool::Git, Platform::Windows),
        vec![
            vec!["winget", "install", "--id", "Git.Git", "-e"],
            vec!["scoop", "install", "git"]
        ]
    );
    assert_eq!(
        commands(Tool::Gh, Platform::Windows)[0],
        vec!["winget", "install", "--id", "GitHub.cli", "-e"]
    );
    let notes = guide(Tool::Gh, Platform::Windows).notes;
    assert!(notes.iter().any(|note| note.contains("new terminal")));
    assert!(notes.iter().any(|note| note.contains("restart")));
}

#[test]
fn linux_matches_the_distro_it_detects() {
    let debian = Platform::Linux {
        family: LinuxFamily::Debian,
    };
    assert_eq!(
        commands(Tool::Git, debian)[0],
        vec!["sudo", "apt", "install", "git"]
    );
    assert!(urls(Tool::Gh, debian)
        .iter()
        .any(|url| url.contains("install_linux")));

    assert_eq!(
        commands(
            Tool::Gh,
            Platform::Linux {
                family: LinuxFamily::Fedora
            }
        )[0],
        vec!["sudo", "dnf", "install", "gh"]
    );
    assert_eq!(
        commands(
            Tool::Gh,
            Platform::Linux {
                family: LinuxFamily::Arch
            }
        )[0],
        vec!["sudo", "pacman", "-S", "github-cli"]
    );
    let generic = urls(
        Tool::Gh,
        Platform::Linux {
            family: LinuxFamily::Other,
        },
    );
    assert!(generic.iter().any(|url| url.contains("cli/cli/releases")));
}

#[test]
fn the_distro_comes_from_os_release() {
    assert_eq!(
        linux_family_from_os_release("ID=ubuntu\nID_LIKE=debian\n"),
        LinuxFamily::Debian
    );
    assert_eq!(
        linux_family_from_os_release("ID=\"fedora\"\n"),
        LinuxFamily::Fedora
    );
    assert_eq!(
        linux_family_from_os_release("ID=manjaro\nID_LIKE=arch\n"),
        LinuxFamily::Arch
    );
    assert_eq!(linux_family_from_os_release(""), LinuxFamily::Other);
}

#[test]
fn instructions_are_data_and_the_command_string_is_only_for_display() {
    let option = &guide(Tool::Git, Platform::Windows).options[0];
    assert_eq!(
        option.display_command().as_deref(),
        Some("winget install --id Git.Git -e")
    );
    assert_eq!(option.command.as_ref().unwrap().len(), 5);
    assert_eq!(
        auth_login_hint().command.unwrap(),
        vec!["gh", "auth", "login"]
    );
}

#[test]
fn every_platform_answers_for_both_tools() {
    for platform in [
        Platform::MacOs,
        Platform::Windows,
        Platform::Linux {
            family: LinuxFamily::Other,
        },
        Platform::Other,
    ] {
        let pair = guides(platform);
        assert_eq!(pair.len(), 2);
        for entry in pair {
            assert!(!entry.options.is_empty());
            assert!(!entry.notes.is_empty());
            assert!(entry
                .options
                .iter()
                .all(|option| option.command.is_some() || option.url.is_some()));
        }
    }
}
