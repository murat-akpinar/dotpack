//! Fonts, GTK themes, icon themes and cursors — the three places the chain can end.
//!
//! A rice whose Nerd Font is missing renders every icon in the bar as a box, so this is
//! not optional decoration. design.md §5.2.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::paths;
use crate::pkg::{self, FileSearch};

/// `key` → the role it fills in `components`.
/// A bare `font` key is deliberately absent: in QML `font: hiddenSummary.font` is an
/// expression, and asking fontconfig about it produces a warning about a font nobody
/// ever named. The three keys below cover kitty, css and gtk.
const FONT_KEYS: &[&str] = &["font_family", "font-family", "gtk-font-name"];
const THEME_KEYS: &[(&str, &str, &str)] = &[
    ("gtk-theme-name", "gtk_theme", "themes"),
    ("gtk-icon-theme-name", "icons", "icons"),
    ("gtk-cursor-theme-name", "cursor", "icons"),
    ("cursor_theme", "cursor", "icons"),
];

#[derive(Debug, PartialEq)]
pub enum Source {
    /// Steps 1 and 2: a package owns the file, or a repo ships it.
    Package(String),
    /// Step 3, and a real answer rather than a fallback: the files go into the bundle,
    /// `apply` runs `fc-cache -f`, and the receiver needs no network. *The bundle is the
    /// download.*
    Ship(PathBuf),
    /// fc-match fell back to something else, so it is not installed here at all.
    Missing,
}

#[derive(Debug, PartialEq)]
pub struct Finding {
    pub role: &'static str,
    /// Exactly as the config writes it, size and weight included.
    pub name: String,
    pub source: Source,
    pub reason: String,
}

pub fn scan(files: &[PathBuf]) -> (Vec<Finding>, Vec<String>) {
    let mut findings: Vec<Finding> = Vec::new();
    let mut warnings = Vec::new();

    for file in files {
        let Some(text) = super::refs::read_text(file) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            let reason = format!("{}:{}", paths::contract(file), index + 1);
            let Some((key, value)) = setting(line) else {
                continue;
            };

            let finding = if FONT_KEYS.contains(&key) {
                Some(Finding {
                    role: font_role(file),
                    source: font_source(&value, &mut warnings),
                    name: value,
                    reason,
                })
            } else {
                THEME_KEYS
                    .iter()
                    .find(|(k, _, _)| *k == key)
                    .map(|(_, role, kind)| Finding {
                        role,
                        source: theme_source(&value, kind, &mut warnings),
                        name: value,
                        reason,
                    })
            };
            if let Some(finding) = finding
                && !findings
                    .iter()
                    .any(|f| f.role == finding.role && f.name == finding.name)
            {
                findings.push(finding);
            }
        }
    }
    (findings, warnings)
}

// --- the three steps start ---

/// `fc-match` **silently falls back**, so the family it returns has to be compared with
/// the one that was asked for. Without that comparison a missing font looks installed.
fn font_source(request: &str, warnings: &mut Vec<String>) -> Source {
    let Some(output) = run("fc-match", &[request, "--format", "%{family}\n%{file}"]) else {
        return Source::Missing;
    };
    let mut lines = output.lines();
    let families = lines.next().unwrap_or_default();
    let file = PathBuf::from(lines.next().unwrap_or_default());

    let asked = request.to_ascii_lowercase();
    // fc-match answers with every alias of the family it picked.
    let matched = families.split(',').any(|family| {
        let family = family.trim().to_ascii_lowercase();
        !family.is_empty() && (asked.starts_with(&family) || asked.contains(&family))
    });
    if !matched {
        return Source::Missing;
    }
    owned_or_shipped(&file, warnings)
}

fn theme_source(name: &str, kind: &str, warnings: &mut Vec<String>) -> Source {
    let directories = [
        PathBuf::from("/usr/share").join(kind),
        paths::local().join("share").join(kind),
        paths::home().join(format!(".{kind}")),
    ];
    let Some(directory) = directories
        .iter()
        .map(|d| d.join(name))
        .find(|d| d.is_dir())
    else {
        return Source::Missing;
    };
    // A theme directory can have several owners (`Adwaita` is both the icon theme and
    // the cursors); `index.theme` belongs to exactly one.
    let marker = directory.join("index.theme");
    let probe = if marker.is_file() {
        marker
    } else {
        directory.clone()
    };
    match pkg::owner(&probe) {
        Some(package) => Source::Package(package),
        None => owned_or_shipped(&directory, warnings),
    }
}

/// Steps 1 → 2 → 3, in that order. Step 2 is where most hand-installed fonts land: the
/// user installed by hand something the repos carry all along.
fn owned_or_shipped(path: &Path, warnings: &mut Vec<String>) -> Source {
    if let Some(package) = pkg::owner(path) {
        return Source::Package(package);
    }
    let basename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    match pkg::ships_file(&basename) {
        FileSearch::Ships(package) => Source::Package(package),
        FileSearch::Nothing => Source::Ship(path.to_path_buf()),
        FileSearch::NoDatabase => {
            let warning = format!(
                "`pacman -Fy` has never run here, so {basename} is shipped as a file — a \
                 repo may well carry it"
            );
            if !warnings.contains(&warning) {
                warnings.push(warning);
            }
            Source::Ship(path.to_path_buf())
        }
    }
}

// --- the three steps end ---

/// `key = value`, `key: value;` and `key value` all occur, across ini, css and kitty.
fn setting(line: &str) -> Option<(&str, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
        return None;
    }
    // The key ends at the first space, `=` or `:`, whichever comes first. kitty ≥ 0.35
    // writes `font_family family="X" style="Y"`, and splitting on `=` first takes the key
    // to be `font_family      family`.
    let end = line.find(|c: char| c.is_whitespace() || c == '=' || c == ':')?;
    let (key, rest) = line.split_at(end);
    let rest = rest.trim_start().trim_start_matches(['=', ':']).trim();
    // ...and that value is itself a `family="…"`.
    let rest = match rest.strip_prefix("family=") {
        Some(family) => family.split('"').nth(1).unwrap_or(family),
        None => rest,
    };
    let value = rest
        .split(',') // css fallback lists: the first family is the one that is wanted
        .next()?
        .trim()
        .trim_end_matches(';')
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .to_string();
    (!value.is_empty()).then_some((key.trim(), value))
}

fn font_role(file: &Path) -> &'static str {
    let path = file.to_string_lossy();
    if path.contains("kitty") || path.contains("foot") || path.contains("alacritty") {
        "font_terminal"
    } else if path.contains("gtk-") {
        "font_system"
    } else {
        "font"
    }
}

fn run(binary: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(binary).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_across_three_file_formats() {
        assert_eq!(
            setting("font_family CaskaydiaMono Nerd Font Mono").unwrap(),
            ("font_family", "CaskaydiaMono Nerd Font Mono".to_string())
        );
        assert_eq!(
            setting("gtk-theme-name=adw-gtk3").unwrap(),
            ("gtk-theme-name", "adw-gtk3".to_string())
        );
        assert_eq!(
            setting("  font-family: \"JetBrains Mono\", monospace;").unwrap(),
            ("font-family", "JetBrains Mono".to_string())
        );
        assert_eq!(
            setting("font_family      family=\"CaskaydiaMono Nerd Font Mono\" style=\"SemiBold\"")
                .unwrap(),
            ("font_family", "CaskaydiaMono Nerd Font Mono".to_string()),
            "kitty >= 0.35"
        );
        assert_eq!(setting("# gtk-theme-name=commented"), None);
    }

    /// Against the real fontconfig on this machine: a name nothing provides must not come
    /// back as installed just because fc-match answered.
    #[test]
    fn a_missing_font_is_not_silently_accepted() {
        let mut warnings = Vec::new();
        assert_eq!(
            font_source("Dotpack No Such Font", &mut warnings),
            Source::Missing
        );
    }
}
