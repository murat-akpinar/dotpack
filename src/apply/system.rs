//! The parts of a switch that are not files: the font cache, user services, and telling
//! the window manager to re-read its config.

use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;

use crate::manifest::Wm;
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

/// True when the command existed and exited 0. Its own output is left alone: the tool
/// does not reimplement anyone else's progress reporting.
fn ok(cmd: &mut Command) -> bool {
    cmd.status().map(|s| s.success()).unwrap_or(false)
}
