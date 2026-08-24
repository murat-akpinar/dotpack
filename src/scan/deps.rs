//! `config line → command name → PATH → pacman → package → source`. design.md §5.
//!
//! This produces **suggestions, it does not decide**. The goal is to replace "remember
//! 40 packages from scratch" with "weed 5 lines out of the 45 offered".

use std::collections::{BTreeMap, BTreeSet};
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
    // The toolchain. A rice never needs an assembler at runtime, and `as`, `not` and
    // `info` are all real binaries that a stray word in a quoted jq program lands on.
    "binutils",
    "llvm",
    "clang",
    "gcc",
    "texinfo",
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

/// A ticked directory is evidence on its own. Nothing in `kitty.conf` launches kitty, so
/// without this `collect kitty` writes a bundle that ships a config for a program it never
/// installs — and the receiver gets the config, not the terminal.
///
/// Cheap and safe because the directory was *chosen*: the name has to resolve to a real
/// binary through the same chain [`scan`] uses, so `~/.config/hypr` suggests nothing.
pub fn from_selection(directories: &[String], warnings: &mut Vec<String>) -> Vec<Suggestion> {
    let foreign = pkg::foreign();
    directories
        .iter()
        .filter_map(|name| {
            let path = pkg::which(name)?;
            let (package, note) = resolve(name, &path, warnings)?;
            (!NOISE.contains(&package.as_str())).then(|| Suggestion {
                aur: foreign.contains(&package),
                package,
                reason: format!("~/.config/{name} is in the bundle"),
                note,
            })
        })
        .collect()
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
    let texts: Vec<(&PathBuf, String)> = files
        .iter()
        .filter_map(|file| Some((file, super::refs::read_text(file)?)))
        .collect();
    // `$terminal = kitty` lives in variables.conf and `bind = …, exec, $terminal` in
    // keybindings.conf, so the table is built over the whole selection before anything is
    // read out of it. Every default sway config hides its terminal behind `set $term`.
    let variables = variables_in(&texts);

    let mut found = BTreeMap::new();
    for (file, text) in &texts {
        // The user's own scripts carry dependencies too, and there are usually more of
        // them than there are exec lines.
        // A shell script only. `#!/usr/bin/env python` read in first-token mode turns
        // `if not os.path.exists(d):` into a suggestion of llvm, which ships /usr/bin/not.
        let shell = ["sh", "bash", "zsh", "fish"];
        let script = file
            .extension()
            .is_some_and(|e| shell.iter().any(|s| e == *s))
            || text
                .lines()
                .next()
                .is_some_and(|l| l.starts_with("#!") && shell.iter().any(|s| l.ends_with(s)));
        // A rice's scripts define `info()`, `error()`, `log()` — names that are also real
        // binaries (texinfo ships /usr/bin/info). Without this, every call to one of them
        // suggests a package the rice has never needed.
        let functions = functions_in(text);
        for (index, line) in text.lines().enumerate() {
            let line = substitute(line, &variables);
            for command in line_commands(&line, script, rules) {
                if functions.contains(&command) {
                    continue;
                }
                found
                    .entry(command)
                    .or_insert_with(|| format!("{}:{}", paths::contract(file), index + 1));
            }
        }
    }
    found
}

/// `$terminal = kitty` (hyprland) and `set $term foot` (sway, i3): one line names a
/// command and every binding below refers to it by variable only.
///
/// Longest name first — substituting `$term` inside `$terminal` would otherwise leave a
/// `footinal` behind.
fn variables_in(texts: &[(&PathBuf, String)]) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = texts
        .iter()
        .flat_map(|(_, text)| text.lines())
        .filter_map(definition)
        .collect();
    found.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
    found
}

fn definition(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    let line = line.strip_prefix("set ").unwrap_or(line);
    let rest = line.strip_prefix('$')?;
    let (name, value) = match rest.split_once('=') {
        Some(pair) => pair,
        // sway and i3 spell it without one: `set $term foot`.
        None => rest.split_once(char::is_whitespace)?,
    };
    let name = name.trim();
    let value = value.trim();
    (!name.is_empty() && !value.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .then(|| (format!("${name}"), value.to_string()))
}

fn substitute(line: &str, variables: &[(String, String)]) -> String {
    let mut out = line.to_string();
    if !out.contains('$') {
        return out;
    }
    for (name, value) in variables {
        out = out.replace(name.as_str(), value);
    }
    out
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
        // `$(` opens a new command: in `info=$(bluetoothctl info "$mac")` the command is
        // bluetoothctl, and the `info` in front of it is a variable being assigned.
        .flat_map(|value| value.split(['|', ';', '&', '`']))
        .flat_map(|value| value.split("$("))
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

/// `name() {` and `function name {`, which is every shell function definition that
/// matters here.
fn functions_in(text: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in text.lines() {
        let line = line.trim();
        let line = line.strip_prefix("function ").unwrap_or(line);
        if let Some((name, rest)) = line.split_once('(')
            && rest.trim_start().starts_with(')')
            && !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            names.insert(name.trim().to_string());
        }
    }
    names
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
    fn a_ticked_directory_is_evidence() {
        let mut warnings = Vec::new();
        assert!(from_selection(&["not-a-real-command".to_string()], &mut warnings).is_empty());
        // The chain is the machine's, so the assertion only holds where kitty is one.
        if pkg::which("kitty").is_some() {
            let found = from_selection(&["kitty".to_string()], &mut warnings);
            assert_eq!(found[0].package, "kitty");
            assert_eq!(found[0].reason, "~/.config/kitty is in the bundle");
        }
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
            ["grim", "slurp", "satty"],
            "three, not two: the command inside `$(…)` is a dependency like any other"
        );
    }

    /// The default sway config names its terminal exactly once, in a `set` line, and the
    /// binding that launches it never spells the command out. The same holds for
    /// hyprland's `$terminal`, which is why this is not a sway-only patch.
    #[test]
    fn a_variable_carries_the_command() {
        let table = |text: &str| {
            let path = PathBuf::from("config");
            variables_in(&[(&path, text.to_string())])
        };
        let sway = table("set $mod Mod4\nset $term foot\n");
        assert_eq!(
            line_commands(
                &substitute("bindsym $mod+Return exec $term", &sway),
                false,
                &super::super::wm::rules(Wm::Sway)
            ),
            ["foot"]
        );
        let hypr = table("$mainMod = SUPER\n$terminal = kitty\n");
        assert_eq!(
            line_commands(
                &substitute("bind = $mainMod, T, exec, $terminal", &hypr),
                false,
                &super::super::wm::rules(Wm::Hyprland)
            ),
            ["kitty"]
        );
        // A colour is a variable too, and substituting it must stay harmless.
        let colors = table("$active_border = rgba(cfbdfeee)");
        assert!(
            line_commands(
                &substitute("col.active_border = $active_border", &colors),
                false,
                &super::super::wm::rules(Wm::Hyprland)
            )
            .is_empty()
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
