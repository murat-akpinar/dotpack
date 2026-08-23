//! Every path the tool uses starts here.
//!
//! `HOME` is read in this file and **nowhere else** — M1's acceptance test runs the whole
//! switch cycle against a temporary `HOME`, which is impossible once `env::var` is spread
//! across modules.

use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    // Unset HOME is not a state the tool can do anything sensible with.
    PathBuf::from(std::env::var("HOME").expect("HOME is not set"))
}

pub fn config() -> PathBuf {
    home().join(".config")
}

pub fn local() -> PathBuf {
    home().join(".local")
}

/// Where bundles live: `~/.local/share/dotpack/bundles/`.
pub fn store() -> PathBuf {
    local().join("share/dotpack/bundles")
}

/// The link ledger — the only state file.
pub fn state_file() -> PathBuf {
    local().join("state/dotpack/state.toml")
}

pub fn backups() -> PathBuf {
    local().join("state/dotpack/backups")
}

/// `~/x` → `$HOME/x`. The ledger and `assets[].dest` store the `~` form.
pub fn expand(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => PathBuf::from(p),
    }
}

/// The inverse: `$HOME/x` → `~/x`, so a ledger written under one HOME still reads.
pub fn contract(p: &Path) -> String {
    match p.strip_prefix(home()) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_round_trip() {
        unsafe { std::env::set_var("HOME", "/tmp/dp-home") }
        assert_eq!(
            expand("~/.config/hypr"),
            PathBuf::from("/tmp/dp-home/.config/hypr")
        );
        assert_eq!(contract(&expand("~/.config/hypr")), "~/.config/hypr");
        // Absolute paths outside HOME survive both directions untouched.
        assert_eq!(expand("/usr/share/x"), PathBuf::from("/usr/share/x"));
        assert_eq!(contract(Path::new("/usr/share/x")), "/usr/share/x");
    }
}
