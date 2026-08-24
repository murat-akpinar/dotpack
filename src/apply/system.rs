//! The parts of a switch that are not files: the font cache, user services, telling the
//! window manager to re-read its config, and running the bundle's hooks.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;

use crate::manifest::{Mode, Wm};
use crate::paths;

/// A font nothing has indexed does not exist as far as every running application is
/// concerned, so this is not cosmetic. Only run when something actually landed there.
pub fn refresh_fonts(touched: &[PathBuf], notes: &mut Vec<String>) -> Result<()> {
    let fonts = paths::local().join("share/fonts");
    if !touched.iter().any(|t| t.starts_with(&fonts)) {
        return Ok(());
    }
    if !ok(Command::new("fc-cache").arg("-f")) {
        notes.push("fc-cache -f failed — new fonts stay invisible until it runs".into());
    }
    Ok(())
}

/// User units only. A bundle from someone else's repo does not get to enable a root
/// service; that is what a reviewed `post_install` hook is for.
pub fn services(enable: &[String], disable: &[String], notes: &mut Vec<String>) {
    // Stopping alone is not enough — an enabled unit comes straight back at next login.
    for unit in disable {
        if !ok(Command::new("systemctl").args(["--user", "disable", "--now", unit])) {
            notes.push(format!("systemctl --user disable --now {unit} failed"));
        }
    }
    for unit in enable {
        if !ok(Command::new("systemctl").args(["--user", "enable", "--now", unit])) {
            notes.push(format!("systemctl --user enable --now {unit} failed"));
        }
    }
}

/// Reload the running WM. A bundle for a WM that is not the one running gets no reload —
/// there is nothing to reload into, and saying "log out" is the honest answer.
pub fn reload(wm: Wm, notes: &mut Vec<String>) {
    let (binary, args): (&str, &[&str]) = match wm {
        Wm::Hyprland => ("hyprctl", &["reload"]),
        Wm::Sway => ("swaymsg", &["reload"]),
        Wm::I3 => ("i3-msg", &["reload"]),
    };
    if !ok(Command::new(binary).args(args)) {
        notes.push(format!(
            "{binary} {} did not run — log out and back in to pick the config up",
            args.join(" ")
        ));
    }
}

/// Someone else's script, from someone else's repo. It has already been shown and
/// approved by the time it gets here (invariant 5).
///
/// A non-zero exit is a warning and nothing more — not even for `pre_install`, which runs
/// before the packages: a hook that could not add a repo is not a reason to abandon a
/// switch that has not started ([manifest.md](../docs/manifest.md) § hooks).
pub fn hook(root: &Path, relative: &str, mode: Mode, notes: &mut Vec<String>) {
    let script = root.join(relative);
    // Rices ship executable scripts and their shebang is the right interpreter; `sh` is
    // only for the ones whose exec bit did not survive the trip.
    let mut command = match executable(&script) {
        true => Command::new(&script),
        false => {
            let mut sh = Command::new("sh");
            sh.arg(&script);
            sh
        }
    };
    command
        .current_dir(root)
        .env("DP_BUNDLE_DIR", root)
        .env("DP_MODE", format!("{mode:?}").to_lowercase());
    if !ok(&mut command) {
        notes.push(format!("hook {relative} failed — nothing was rolled back"));
    }
}

fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// True when the command existed and exited 0. Its own output is left alone: the tool
/// does not reimplement anyone else's progress reporting.
fn ok(cmd: &mut Command) -> bool {
    cmd.status().map(|s| s.success()).unwrap_or(false)
}
