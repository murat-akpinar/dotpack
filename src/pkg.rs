//! pacman and the AUR helper — the only module that installs anything system-wide.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::manifest::Manifest;

/// Searched in this order; the first one present installs the AUR set, whichever field
/// the bundle happened to write it in.
const HELPERS: &[&str] = &["paru", "yay", "pikaur", "trizen"];

#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    /// A repo has these — `sudo pacman -S --needed`.
    pub repo: Vec<String>,
    /// The declared AUR set plus everything no repo has — the helper installs both.
    pub aur: Vec<String>,
    /// A subset of `aur`: names the bundle put in `packages.pacman` that no repo carries.
    /// Kept apart only so the summary can say why they are going to the helper.
    pub unknown: Vec<String>,
    /// `None` → nothing in `aur` can be installed, and every one of them comes back in
    /// `install`'s failure list.
    pub helper: Option<String>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.repo.is_empty() && self.aur.is_empty()
    }
}

/// What this bundle still needs on this machine.
pub fn plan(manifest: &Manifest) -> Result<Plan> {
    let p = &manifest.packages;
    // yay and paru are one set: the field only records which helper the author uses.
    let mut aur: Vec<String> = p.yay.iter().chain(&p.paru).cloned().collect();
    aur.sort();
    aur.dedup();

    let mut repo = unsatisfied(&p.pacman)?;
    let mut aur = unsatisfied(&aur)?;

    // A single name no repo has would make `pacman -S` refuse the whole transaction, so
    // it is moved to the helper — which searches the AUR as well — rather than left in.
    let unknown = not_in_repos(&repo)?;
    repo.retain(|name| !unknown.contains(name));
    aur.extend(unknown.iter().cloned());

    Ok(Plan {
        repo,
        aur,
        unknown,
        helper: helper(),
    })
}

/// Names no repo carries. `pacman -Si` resolves **provides** and keeps going past the
/// unknown ones, printing a `was not found` line for each; `-Ss` searches neither
/// provides nor exactly, and concluding "no such package" from it is wrong (design.md §5).
fn not_in_repos(names: &[String]) -> Result<Vec<String>> {
    if names.is_empty() {
        return Ok(Vec::new());
    }
    let out = Command::new("pacman")
        .env("LC_ALL", "C") // the message below is matched, so it must not be translated
        .arg("-Si")
        .args(names)
        .output()
        .context("pacman not found — dotpack is Arch only")?;
    Ok(String::from_utf8_lossy(&out.stderr)
        .lines()
        .filter(|l| l.contains("was not found"))
        .filter_map(|l| l.split('\'').nth(1))
        .map(str::to_string)
        .collect())
}

/// Install everything in the plan, then report **what is still missing afterwards**.
///
/// That report is exact without parsing a line of pacman's output, and it is the whole
/// failure story a switch needs: packages are never removed, so "installed the rest,
/// these three failed" is a working switch with a named gap (TODO.md Phase 0).
pub fn install(plan: &Plan) -> Result<Vec<String>> {
    if !plan.repo.is_empty() {
        // `-S --needed`, never `-Syu`: a dotfile installer does not upgrade someone's
        // system behind their back (invariant 8).
        run(Command::new("sudo")
            .args(["pacman", "-S", "--needed"])
            .args(&plan.repo))?;
    }
    // No helper: the AUR set comes straight back as unsatisfied below, named.
    if let Some(helper) = &plan.helper
        && !plan.aur.is_empty()
    {
        run(Command::new(helper)
            .args(["-S", "--needed"])
            .args(&plan.aur))?;
    }
    let all: Vec<String> = plan.repo.iter().chain(&plan.aur).cloned().collect();
    unsatisfied(&all)
}

/// Which of these are not satisfied on this machine.
///
/// `pacman -T` resolves **provides**, so a bundle asking for `quickshell` is satisfied by
/// the installed `noctalia-qs` — a set difference against `pacman -Qq` would reinstall it
/// and hit a conflict (design.md §5).
fn unsatisfied(packages: &[String]) -> Result<Vec<String>> {
    if packages.is_empty() {
        return Ok(Vec::new());
    }
    let out = Command::new("pacman")
        .arg("-T")
        .args(packages)
        .output()
        .context("pacman not found — dotpack is Arch only")?;
    match out.status.code() {
        Some(0) => Ok(Vec::new()),
        // 127 is pacman's "some of these are unsatisfied", not a failure.
        Some(127) => Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()),
        _ => bail!(
            "pacman -T failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ),
    }
}

// --- requires start ---

/// `requires = { hyprland = ">=0.56" }`. **Warns, never blocks** — someone may want the
/// files anyway, and that is their call (manifest.md).
///
/// A package that is not installed yet says nothing: the install is about to bring it in,
/// and pacman hands out the newest version there is.
pub fn requires_warnings(requires: &BTreeMap<String, String>) -> Vec<String> {
    let mut warnings = Vec::new();
    for (name, spec) in requires {
        let Some(have) = installed_version(name) else {
            continue;
        };
        let wanted = spec.trim_start_matches(['>', '=', ' ']);
        match at_least(&have, wanted) {
            Some(true) => {}
            Some(false) => warnings.push(format!("{name} {spec} — this machine has {have}")),
            None => warnings.push(format!(
                "{name} {spec} cannot be compared with the installed {have}"
            )),
        }
    }
    warnings
}

fn installed_version(name: &str) -> Option<String> {
    let out = Command::new("pacman").args(["-Q", name]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&out.stdout);
    Some(upstream(line.split_whitespace().nth(1)?))
}

/// `pacman -Q` prints `hyprland 1:0.56.0-2`: the epoch and the pkgrel are Arch's, not the
/// upstream version, so both come off before anything is compared.
fn upstream(version: &str) -> String {
    let no_epoch = version.split_once(':').map_or(version, |(_, rest)| rest);
    no_epoch
        .rsplit_once('-')
        .map_or(no_epoch, |(v, _)| v)
        .into()
}

/// Field by field, **as integers**: `0.9` against `0.56` is why a string comparison is
/// not acceptable. A non-numeric field anywhere means "cannot compare" — `None`.
fn at_least(have: &str, wanted: &str) -> Option<bool> {
    let fields = |v: &str| {
        v.split('.')
            .map(|f| f.parse::<u64>().ok())
            .collect::<Option<Vec<u64>>>()
    };
    let (have, wanted) = (fields(have)?, fields(wanted)?);
    // A missing field is a zero: 0.56 is 0.56.0, and neither is newer than the other.
    for index in 0..have.len().max(wanted.len()) {
        let (h, w) = (
            have.get(index).copied().unwrap_or(0),
            wanted.get(index).copied().unwrap_or(0),
        );
        if h != w {
            return Some(h > w);
        }
    }
    Some(true)
}

// --- requires end ---

/// The package that owns a file. It can answer with a name that is **not** the command
/// and that is not an error: `/usr/bin/quickshell` is owned by `noctalia-qs`, which
/// *provides* `quickshell` (design.md §5).
pub fn owner(path: &Path) -> Option<String> {
    let out = Command::new("pacman").arg("-Qoq").arg(path).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Can this name be installed from a repo anywhere? `pacman -Si` resolves provides; `-Ss`
/// searches names and descriptions only, so concluding "no such package" from it is wrong.
pub fn in_repos(name: &str) -> bool {
    not_in_repos(std::slice::from_ref(&name.to_string())).is_ok_and(|missing| missing.is_empty())
}

/// Installed from outside the repos — the AUR set on this machine.
pub fn foreign() -> BTreeSet<String> {
    Command::new("pacman")
        .args(["-Qqem"])
        .output()
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// The package that *ships* a file, looked up by basename — the answer for a font or a
/// binary installed by hand that a repo carries all along.
pub enum FileSearch {
    Ships(String),
    Nothing,
    /// `pacman -Fy` has never been run here, so the question cannot be asked.
    NoDatabase,
}

pub fn ships_file(basename: &str) -> FileSearch {
    let Ok(out) = Command::new("pacman")
        .env("LC_ALL", "C")
        .arg("-F")
        .arg(basename)
        .output()
    else {
        return FileSearch::Nothing;
    };
    if String::from_utf8_lossy(&out.stderr).contains("-Fy") {
        return FileSearch::NoDatabase;
    }
    // `repo/name version\n    usr/bin/thing` — the package name is the second field of
    // the first line.
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().next())
        .and_then(|full| full.split('/').nth(1))
        .map(|name| FileSearch::Ships(name.to_string()))
        .unwrap_or(FileSearch::Nothing)
}

/// `command -v`, without a shell.
pub fn which(command: &str) -> Option<PathBuf> {
    std::env::var("PATH").ok()?.split(':').find_map(|dir| {
        let candidate = Path::new(dir).join(command);
        candidate.is_file().then_some(candidate)
    })
}

pub fn helper() -> Option<String> {
    HELPERS
        .iter()
        .find(|h| Command::new(h).arg("--version").output().is_ok())
        .map(|h| h.to_string())
}

/// pacman prints its own progress; we only care whether it worked.
fn run(cmd: &mut Command) -> Result<bool> {
    Ok(cmd.status()?.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Invariant 8, as a grep that runs. The needle is built at runtime so that this
    /// file does not contain the very string it forbids.
    #[test]
    fn never_upgrades_the_system() {
        let forbidden = format!("\"-S{}\"", "yu");
        assert!(!include_str!("pkg.rs").contains(&forbidden));
    }

    /// `0.9` against `0.56` is the case a string comparison gets wrong.
    #[test]
    fn versions_compare_as_integers() {
        assert_eq!(at_least("0.56", "0.9"), Some(true));
        assert_eq!(at_least("0.9", "0.56"), Some(false));
        assert_eq!(at_least("0.56.0", "0.56"), Some(true));
        assert_eq!(at_least("1.0", "1.0.1"), Some(false));
        assert_eq!(at_least("0.50.1-rc", "0.50"), None);
    }

    /// pacman answers with an epoch and a pkgrel; neither is the upstream version.
    #[test]
    fn the_installed_version_is_stripped() {
        assert_eq!(upstream("1:0.56.0-2"), "0.56.0");
        assert_eq!(upstream("0.56.0"), "0.56.0");
        // A `-git` build's version genuinely cannot be compared field by field, and
        // saying so is the right answer — this machine's own pacman is one.
        assert_eq!(at_least(&upstream("7.1.0.r9.g54d9411-1"), "7.1"), None);
        assert_eq!(installed_version("pacman-not-a-real-package"), None);
        assert!(installed_version("pacman").is_some());
    }

    #[test]
    fn empty_lists_never_shell_out() {
        assert_eq!(unsatisfied(&[]).unwrap(), Vec::<String>::new());
        assert_eq!(not_in_repos(&[]).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn a_name_no_repo_has_goes_to_the_helper() {
        let names = [
            "kitty".to_string(),
            "dotpack-not-a-real-package".to_string(),
        ];
        assert_eq!(
            not_in_repos(&names).unwrap(),
            ["dotpack-not-a-real-package"]
        );
    }

    /// Read-only, and the answer is the same on every Arch machine.
    #[test]
    fn deptest_knows_what_is_missing() {
        let asked = [
            "pacman".to_string(),
            "dotpack-not-a-real-package".to_string(),
        ];
        assert_eq!(unsatisfied(&asked).unwrap(), ["dotpack-not-a-real-package"]);
    }
}
