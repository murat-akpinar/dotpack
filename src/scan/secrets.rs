//! Leaking a token through a shared dotfile is this project's biggest risk, so this is
//! the one scan that does not get simplified (invariant 4). design.md §6.
//!
//! Two layers: paths that are never added by default, and a content scan over the files
//! that are. The content scan cannot stop at `collect` — in symlink mode
//! `~/.config/fish/config.fish` *is* the file inside a git repo that is probably public,
//! so a token added the day after is seen by nothing unless `ls` looks too.

use std::path::{Path, PathBuf};

// --- deny-list start ---
/// Never added to a bundle by default. Shell history is on this list because a **public**
/// repo that was examined shipped `zsh/private_dot_histfile`: chezmoi's `private_` prefix
/// means `chmod 600`, it does not mean "not published".
pub const DENY: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws",
    ".kube",
    ".docker/config.json",
    ".netrc",
    ".config/gh",
    ".config/sops",
    ".config/age",
    ".config/rclone",
    ".config/mozilla",
    ".config/Code",
    ".config/discord",
    ".local/share/keyrings",
    "*.pem",
    "*.key",
    "id_rsa*",
    "id_ed25519*",
    "*histfile*",
    ".bash_history",
    ".zsh_history",
    ".python_history",
    ".node_repl_history",
];
// --- deny-list end ---

// --- content patterns start ---
/// Fixed substrings, matched at a token boundary so that `disk-usage` is not an OpenAI
/// key. ponytail: substring matching, no `regex` crate; if too much escapes, that is the
/// upgrade.
const TOKENS: &[(&str, &str)] = &[
    ("ghp_", "GitHub personal access token"),
    ("github_pat_", "GitHub fine-grained token"),
    ("sk-", "OpenAI-style API key"),
    ("AKIA", "AWS access key id"),
    ("xoxb-", "Slack bot token"),
];

/// `key<space>=` assignments, matched case-insensitively.
const ASSIGNMENTS: &[(&str, &str)] = &[
    ("password", "password assignment"),
    ("token", "token assignment"),
    ("api_key", "api key assignment"),
    ("api-key", "api key assignment"),
    ("apikey", "api key assignment"),
];
// --- content patterns end ---

#[derive(Debug, PartialEq)]
pub struct Finding {
    pub file: PathBuf,
    pub line: usize,
    pub what: &'static str,
}

/// Which deny-list entry keeps this path out, if any.
pub fn denied(relative: &Path) -> Option<&'static str> {
    let path = relative.to_string_lossy();
    let name = relative.file_name()?.to_string_lossy().to_string();
    DENY.iter().copied().find(|entry| match entry {
        // A path fragment matches anywhere in the path: `.config/gh` covers everything
        // under it.
        e if e.contains('/') => path.contains(e),
        e if e.starts_with('*') && e.ends_with('*') => name.contains(e.trim_matches('*')),
        e if let Some(suffix) = e.strip_prefix('*') => name.ends_with(suffix),
        e if let Some(prefix) = e.strip_suffix('*') => name.starts_with(prefix),
        // Otherwise a whole component: `.ssh` matches `~/.ssh/config`, not `x.sshrc`.
        e => relative.components().any(|c| c.as_os_str() == *e),
    })
}

/// The content scan. Findings are shown in red and **unticked by default** — the user has
/// to tick one deliberately.
pub fn scan(files: &[PathBuf]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for file in files {
        let Ok(bytes) = std::fs::read(file) else {
            continue;
        };
        if bytes.iter().take(1024).any(|b| *b == 0) {
            continue;
        }
        for (index, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
            if let Some(what) = suspicious(line) {
                findings.push(Finding {
                    file: file.clone(),
                    line: index + 1,
                    what,
                });
            }
        }
    }
    findings
}

fn suspicious(line: &str) -> Option<&'static str> {
    if line.contains("BEGIN") && line.contains("PRIVATE KEY") {
        return Some("private key");
    }
    for (token, what) in TOKENS {
        if at_token_start(line, token) {
            return Some(what);
        }
    }
    let lower = line.to_ascii_lowercase();
    for (key, what) in ASSIGNMENTS {
        if let Some(at) = lower.find(key)
            && !starts_inside_a_word(&lower, at)
            && lower[at + key.len()..].trim_start().starts_with('=')
        {
            return Some(what);
        }
    }
    None
}

/// `sk-` is three characters, and a config file full of `disk-usage` findings is a
/// warning screen nobody reads.
fn at_token_start(line: &str, needle: &str) -> bool {
    line.match_indices(needle)
        .any(|(at, _)| !starts_inside_a_word(line, at))
}

fn starts_inside_a_word(line: &str, at: usize) -> bool {
    line[..at]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_list_shapes() {
        let d = |p: &str| denied(Path::new(p));
        assert_eq!(d(".ssh/id_ed25519"), Some(".ssh"));
        assert_eq!(d(".config/gh/hosts.yml"), Some(".config/gh"));
        assert_eq!(d(".config/wireguard/wg0.key"), Some("*.key"));
        assert_eq!(d("zsh/.zsh_history"), Some(".zsh_history"));
        assert_eq!(d("zsh/private_dot_histfile"), Some("*histfile*"));
        assert_eq!(d(".config/hypr/hyprland.conf"), None);
        assert_eq!(
            d(".config/kitty/x.sshrc"),
            None,
            "a component, not a substring"
        );
    }

    #[test]
    fn content_patterns() {
        assert_eq!(
            suspicious("-----BEGIN OPENSSH PRIVATE KEY-----"),
            Some("private key")
        );
        assert!(suspicious("export GH=ghp_16CharactersOrSo").is_some());
        assert!(suspicious("set -x OPENAI_KEY sk-proj-abcdef").is_some());
        assert!(
            suspicious("  Password = hunter2").is_some(),
            "case-insensitive, spaced"
        );
        assert!(suspicious("api-key=abc").is_some());

        // The noise a three-character needle would otherwise make.
        assert_eq!(suspicious("df -h /dev/disk-usage"), None);
        assert_eq!(suspicious("bind = SUPER, T, exec, kitty"), None);
        // A commented-out assignment still ships the value, so it is still a finding.
        assert!(suspicious("# token = ghp_theOneYouForgot").is_some());
        assert_eq!(
            suspicious("# how to set your token"),
            None,
            "no assignment, no finding"
        );
    }
}
