//! pacman and the AUR helper — the only module that installs anything system-wide.

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
