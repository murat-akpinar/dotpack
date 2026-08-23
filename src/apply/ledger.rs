//! `state.toml` — the only state file there is.
//!
//! A clean switch depends entirely on this file being accurate: what is active, what was
//! active before, every link placed, every directory created to place it, and whose hooks
//! have already run.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ledger {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
    /// Where `use -` goes back to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<String>,
    /// Bundles whose hooks have run once. Real hooks append to files, and appending
    /// twice is not undoable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hooks_ran: Vec<String>,
    // An array of tables, so it serializes last.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Link>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    /// Stored in `~/…` form, so a ledger still reads under a different HOME.
    pub target: String,
    pub kind: Kind,
    /// There was a real file here and it was moved into the backups.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adopted_backup: Option<String>,
    /// Directories that did not exist and had to be created to place this link. Removed
    /// on the way out **only if empty** — the user may have put their own files there.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mkdirs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Dir,
    File,
}

impl Ledger {
    /// A missing file is an empty ledger — nothing has ever been activated.
    pub fn load() -> Result<Self> {
        let path = paths::state_file();
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).with_context(|| format!("{}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("{}", path.display())),
        }
    }

    /// Written through a temporary file: a half-written ledger loses track of links that
    /// are already on disk, and there is no second copy to rebuild it from.
    pub fn save(&self) -> Result<()> {
        let path = paths::state_file();
        std::fs::create_dir_all(path.parent().expect("state file has a parent"))?;
        let tmp = path.with_extension("toml.new");
        std::fs::write(&tmp, toml::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

/// `2026-08-23T19:59:57Z`. Shelling out to `date` beats pulling in a date crate or
/// writing civil-calendar arithmetic for one string.
pub fn now() -> String {
    std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let ledger = Ledger {
            active: Some("my-hyprland".into()),
            previous: Some("caelestia".into()),
            activated_at: Some(now()),
            services: vec!["hypridle".into()],
            hooks_ran: vec!["my-hyprland".into()],
            links: vec![Link {
                target: "~/.local/share/fonts/CascadiaMono/x.ttf".into(),
                kind: Kind::File,
                adopted_backup: Some("2026-08-23T14-02-11/.config/fish".into()),
                mkdirs: vec!["~/.local/share/fonts/CascadiaMono".into()],
            }],
        };
        let text = toml::to_string_pretty(&ledger).unwrap();
        assert_eq!(ledger, toml::from_str(&text).unwrap());
        assert!(text.contains("kind = \"file\""), "{text}");
    }

    #[test]
    fn stamp_is_a_timestamp() {
        let stamp = now();
        assert_eq!(stamp.len(), 20, "{stamp}");
        assert!(stamp.ends_with('Z'));
    }
}
