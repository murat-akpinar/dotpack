//! `config line → command name → PATH → pacman → package → source`. design.md §5.
//!
//! This produces **suggestions, it does not decide**. The goal is to replace "remember
//! 40 packages from scratch" with "weed 5 lines out of the 45 offered".

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::manifest::Wm;
use crate::paths;
use crate::pkg::{self, FileSearch};

use super::wm::Rules;

/// Tokens that stand in front of the command that matters. When one is seen, look at the
/// next token — that is the whole of the `uwsm app --` / `sh -c` problem.
const WRAPPERS: &[&str] = &[
    "uwsm",
    "app",
    "sh",
    "bash",
    "zsh",
    "fish",
    "env",
    "exec",
    "sudo",
    "nohup",
    "setsid",
    "systemd-run",
    "hyprctl",
    "swaymsg",
    "i3-msg",
    "dispatch",
    "if",
    "then",
    "else",
    "elif",
    "fi",
    "do",
    "done",
    "while",
    "until",
    "case",
    "esac",
    "command",
    "time",
];

/// Packages whose commands are the shell and the base system, not a rice's dependencies.
/// Filtering on the *owning package* rather than on a list of command names is both
/// shorter and more accurate: it drops `sleep` and `pkill` without naming them, and keeps
/// `notify-send`, which a name list of "obvious" commands usually eats.
const NOISE: &[&str] = &[
    "coreutils",
    "bash",
    "dash",
    "systemd",
    "procps-ng",
    "util-linux",
    "findutils",
    "grep",
    "sed",
    "gawk",
    "which",
    "shadow",
    "sudo",
    "glibc",
    "pacman",
    "filesystem",
    "ncurses",
    "diffutils",
    "gzip",
    "tar",
];

#[derive(Debug, PartialEq)]
pub struct Suggestion {
    pub package: String,
    /// Where it was seen — every suggestion can be traced back to a line.
    pub reason: String,
    pub aur: bool,
    /// Why the package name is not the command name.
    pub note: Option<String>,
}

pub fn scan(files: &[PathBuf], wm: Wm) -> (Vec<Suggestion>, Vec<String>) {
    let rules = super::wm::rules(wm);
    let foreign = pkg::foreign();
    let mut suggestions = Vec::new();
    let mut warnings = Vec::new();

    // ponytail: one `pacman -Qoq` per command. `pacman -Qlq` in bulk is the upgrade if
    // this ever becomes measurably slow.
    for (command, reason) in candidates(files, &rules) {
        let Some(path) = pkg::which(&command) else {
            // Not a binary on this machine: a shell keyword, a function, or a typo.
            continue;
        };
        let Some((package, note)) = resolve(&command, &path, &mut warnings) else {
            continue;
        };
        if NOISE.contains(&package.as_str()) {
            continue;
        }
        if suggestions
            .iter()
            .any(|s: &Suggestion| s.package == package)
        {
            continue;
        }
        suggestions.push(Suggestion {
            aur: foreign.contains(&package),
            package,
            reason,
            note,
        });
    }
    (suggestions, warnings)
}

/// The chain, and the two ways it bends:
///
/// - `-Qoq` can answer with a **provider**: `/usr/bin/quickshell` is owned by
///   `noctalia-qs`. One machine's accident is not what the receiver should install, so
///   the portable name wins when a repo has it.
/// - nothing may own the file at all — `/usr/local/bin/starship`, installed by a
///   `curl | sh` while `extra` has shipped it all along.
fn resolve(
    command: &str,
    path: &Path,
    warnings: &mut Vec<String>,
) -> Option<(String, Option<String>)> {
    match pkg::owner(path) {
        Some(owner) if owner == command => Some((owner, None)),
        Some(owner) if pkg::in_repos(command) => Some((
            command.to_string(),
            Some(format!("installed here by {owner}, which provides it")),
        )),
        Some(owner) => Some((owner, None)),
        None if pkg::in_repos(command) => Some((
            command.to_string(),
            Some(format!(
                "{} belongs to no package, but a repo carries this name",
                path.display()
            )),
        )),
        None => match pkg::ships_file(&file_name(path)) {
            FileSearch::Ships(package) => Some((
                package,
                Some(format!("ships {}, which nothing here owns", path.display())),
            )),
            FileSearch::NoDatabase => {
                let warning = "`pacman -Fy` has never run here, so unowned files cannot be \
                               traced to a package"
                    .to_string();
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
                None
            }
            FileSearch::Nothing => {
                warnings.push(format!(
                    "no package provides {command} — ship it under local/bin/ or declare it \
                     as a manual step"
                ));
                None
            }
        },
    }
}

// --- extraction start ---

/// Command name → where it was first seen.
fn candidates(files: &[PathBuf], rules: &Rules) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    for file in files {
        let Some(text) = super::refs::read_text(file) else {
            continue;
        };
        // The user's own scripts carry dependencies too, and there are usually more of
        // them than there are exec lines.
        let script = file.extension().is_some_and(|e| e == "sh") || text.starts_with("#!");
        for (index, line) in text.lines().enumerate() {
            for command in line_commands(line, script, rules) {
                found
                    .entry(command)
                    .or_insert_with(|| format!("{}:{}", paths::contract(file), index + 1));
            }
        }
    }
    found
}

fn line_commands(line: &str, script: bool, rules: &Rules) -> Vec<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Vec::new();
    }
    let mut values: Vec<&str> = Vec::new();

    if script {
        values.push(trimmed);
    } else {
        let key = trimmed
            .split(|c: char| c.is_whitespace() || c == '=')
            .next()
            .unwrap_or_default();
        if rules.exec_keys.contains(&key) {
            values.push(
                trimmed[key.len()..]
                    .trim_start()
                    .trim_start_matches('=')
                    .trim(),
            );
        }
        // `bind = SUPER, Q, exec, kitty` — the binding's command is not in key position.
        if let Some(at) = trimmed.find(rules.bind_marker) {
            values.push(&trimmed[at + rules.bind_marker.len()..]);
        }
    }

    values
        .iter()
        .flat_map(|value| value.split(['|', ';', '&']))
        .filter_map(bare_command)
        .collect()
}

/// The first token of a segment that is a plain command name. A path is a script the
/// bundle ships (the reference scan's business, not this one's) and a `$` is a variable
/// nobody should guess at.
fn bare_command(segment: &str) -> Option<String> {
    for token in segment.split_whitespace() {
        let token = token.trim_matches(|c: char| "\"'`()[]{}".contains(c));
        if token.is_empty() || token.starts_with('-') || token.contains('=') {
            continue;
        }
        if WRAPPERS.contains(&token) {
            continue;
        }
        if token.contains('/') || token.contains('$') {
            return None;
        }
        return Some(token.to_string());
    }
    None
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

// --- extraction end ---

#[cfg(test)]
mod tests {
    use super::*;

    fn hypr(line: &str) -> Vec<String> {
        line_commands(line, false, &super::super::wm::rules(Wm::Hyprland))
    }

    #[test]
    fn wrappers_are_stepped_over() {
        assert_eq!(hypr("exec-once = uwsm app -- waybar"), ["waybar"]);
        assert_eq!(hypr("exec-once = sh -c 'pkill waybar'"), ["pkill"]);
        assert_eq!(hypr("bind = SUPER, Q, exec, kitty"), ["kitty"]);
        assert_eq!(hypr("exec-once = swayosd-server"), ["swayosd-server"]);
        assert!(hypr("# exec-once = disabled").is_empty());
        assert!(hypr("general { gaps_in = 5 }").is_empty());
    }

    #[test]
    fn a_path_is_a_script_not_a_command() {
        assert!(hypr("exec-once = ~/.config/hypr/scripts/init.sh").is_empty());
        assert!(hypr("exec-once = $HOME/bin/x").is_empty());
    }

    #[test]
    fn pipelines_carry_dependencies_too() {
        assert_eq!(
            line_commands(
                "grim -g \"$(slurp)\" - | satty -f -",
                true,
                &super::super::wm::rules(Wm::Hyprland)
            ),
            ["grim", "satty"]
        );
    }

    #[test]
    fn sway_bindings() {
        let rules = super::super::wm::rules(Wm::Sway);
        assert_eq!(
            line_commands("bindsym $mod+Return exec kitty", false, &rules),
            ["kitty"]
        );
        assert_eq!(
            line_commands("status_command i3status", false, &rules),
            ["i3status"]
        );
    }
}
