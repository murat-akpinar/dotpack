//! `dotfiles.toml` — the only mandatory file in a bundle. Schema: `docs/manifest.md`.
//!
//! This module reads and validates; it never writes. Producing the file is
//! `apply::write`'s job (invariant 1).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::bundle::safe_rel;

/// The schema version this build understands. A higher one warns and keeps going.
pub const SCHEMA: u32 = 1;

// --- default ignore start ---
/// Never collected into a bundle. A manifest's own `ignore` is *added* to this list,
/// it does not replace it. `preview/` is deliberately absent — the video patterns
/// already cover the dead weight, and the screenshot is why the directory exists.
#[allow(dead_code)] // read by collect, M2
pub const DEFAULT_IGNORE: &[&str] = &[
    ".git/",
    "node_modules/",
    "*.mp4",
    "*.gif",
    "*.log",
    "Code/",
    "*/History/",
    "*Trust Tokens*",
    "*.ovpn",
];
// --- default ignore end ---

/// Which pattern keeps this path out of a bundle, if any. **Collect-time only**: at
/// install time `~/.config/hypr` is one directory link and there is no per-file decision
/// left to make.
///
/// A written `ignore` list is *added* to [`DEFAULT_IGNORE`], it does not replace it.
pub fn ignored<'a>(patterns: &'a [String], relative: &Path) -> Option<&'a str> {
    let path = relative.to_string_lossy().replace('\\', "/");
    patterns
        .iter()
        .map(String::as_str)
        .chain(DEFAULT_IGNORE.iter().copied())
        .find(|pattern| matches(pattern, &path))
}

fn matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim_end_matches('/');
    // A leading `*/` means "anywhere below": `*/History/` is VS Code's state directory,
    // wherever it turns up.
    let (pattern, floating) = match pattern.strip_prefix("*/") {
        Some(rest) => (rest, true),
        None => (pattern, false),
    };
    // A bare pattern matches any single component: `*.mp4` any file, `Code` any directory.
    if !floating && !pattern.contains('/') {
        return path.split('/').any(|segment| glob(pattern, segment));
    }
    let segments: Vec<&str> = path.split('/').collect();
    let wanted: Vec<&str> = pattern.split('/').collect();
    // Fewer pattern segments than path segments is a prefix match, so `.git/` covers
    // everything under it.
    (0..if floating { segments.len() } else { 1 }).any(|start| {
        wanted
            .iter()
            .enumerate()
            .all(|(index, p)| segments.get(start + index).is_some_and(|s| glob(p, s)))
    })
}

/// `*` and nothing else. ponytail: if a real glob is ever needed, that is where `glob`
/// earns its place — the default list and the manifests seen so far use only this.
fn glob(pattern: &str, segment: &str) -> bool {
    let mut parts = pattern.split('*');
    let Some(first) = parts.next() else {
        return pattern == segment;
    };
    if !segment.starts_with(first) {
        return false;
    }
    if !pattern.contains('*') {
        return pattern == segment;
    }
    let mut rest = &segment[first.len()..];
    let parts: Vec<&str> = parts.collect();
    for (index, part) in parts.iter().enumerate() {
        if index == parts.len() - 1 {
            return rest.ends_with(part);
        }
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    true
}

// --- types start ---

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default = "default_schema")]
    pub schema: u32,
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    pub wm: Wm,
    #[serde(default = "default_distro")]
    pub distro: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default)]
    pub mode: Mode,
    /// `external` mode only, informational — the tool is never called.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub managed_by: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore: Vec<String>,

    // Everything below serializes as a TOML table, so it has to come after every value.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub requires: BTreeMap<String, String>,
    #[serde(default)]
    pub packages: Packages,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub components: BTreeMap<String, Component>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<Hooks>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<Asset>,
}

#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Wm {
    Hyprland,
    Sway,
    I3,
}

#[derive(Debug, PartialEq, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Symlink,
    /// The bundle ships no files; chezmoi/stow places them. We install packages only.
    External,
}

#[derive(Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Packages {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pacman: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub yay: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paru: Vec<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Component {
    /// Short form: `shell = "fish"`.
    Pkg(String),
    /// Long form: `bar = { pkg = "waybar", theme = "forest" }`.
    Full(ComponentDetail),
}

#[derive(Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct ComponentDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pkg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// "install this yourself" — printed as a manual step, never fetched (invariant 11).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Hooks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_install: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_install: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub src: String,
    pub dest: String,
}

fn default_schema() -> u32 {
    SCHEMA
}
fn default_version() -> String {
    "0.0.0".into()
}
fn default_distro() -> String {
    "arch".into()
}

// --- types end ---

impl Component {
    pub fn pkg(&self) -> Option<&str> {
        match self {
            Component::Pkg(p) => Some(p),
            Component::Full(d) => d.pkg.as_deref(),
        }
    }
}

impl Hooks {
    pub fn paths(&self) -> impl Iterator<Item = &String> {
        self.pre_install.iter().chain(self.post_install.iter())
    }
}

impl Manifest {
    /// Read `<dir>/dotfiles.toml`. A bundle without one is rejected here (invariant 10).
    pub fn load(dir: &Path) -> Result<Self> {
        let file = dir.join("dotfiles.toml");
        let text = std::fs::read_to_string(&file).with_context(|| {
            format!(
                "no dotfiles.toml in {} — not a dotpack bundle",
                dir.display()
            )
        })?;
        toml::from_str(&text).with_context(|| format!("{}", file.display()))
    }

    /// Callers: `apply::write::write_bundle()`, M2. Exercised by the round-trip test.
    #[allow(dead_code)]
    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Hard errors abort the install; warnings are returned for the caller to print.
    /// Anything needing machine state (WM match, `requires` versions) is checked later.
    pub fn validate(&self) -> Result<Vec<String>> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if self.name.is_empty()
            || !self.name.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-')
            })
        {
            errors.push(format!(
                "name `{}` is not a valid store directory name ([a-z0-9._-]+)",
                self.name
            ));
        }

        for hook in self.hooks.iter().flat_map(Hooks::paths) {
            if !safe_rel(hook) {
                errors.push(format!("hook path `{hook}` escapes the bundle"));
            }
        }

        for asset in &self.assets {
            if !safe_rel(&asset.src) {
                errors.push(format!("assets src `{}` escapes the bundle", asset.src));
            }
            if !asset.dest.starts_with('/') && !asset.dest.starts_with('~') {
                errors.push(format!(
                    "assets dest `{}` is neither absolute nor ~-relative",
                    asset.dest
                ));
            }
        }

        if self.schema > SCHEMA {
            warnings.push(format!(
                "schema {} is newer than this build understands ({SCHEMA}) — trying anyway",
                self.schema
            ));
        }
        if self.distro != "arch" {
            warnings.push(format!("distro `{}` is not supported in v1", self.distro));
        }

        let lists = [
            ("pacman", &self.packages.pacman),
            ("yay", &self.packages.yay),
            ("paru", &self.packages.paru),
        ];
        for (field, list) in lists {
            let mut seen = Vec::new();
            for pkg in list {
                if seen.contains(&pkg) {
                    warnings.push(format!("packages.{field} lists `{pkg}` twice"));
                } else {
                    seen.push(pkg);
                }
            }
        }

        // components is descriptive; install logic reads `packages`. So this is a warning
        // and nothing more.
        for (role, component) in &self.components {
            if let Some(pkg) = component.pkg()
                && !lists.iter().any(|(_, l)| l.iter().any(|p| p == pkg))
            {
                warnings.push(format!(
                    "components.{role} names `{pkg}`, which no packages list has"
                ));
            }
        }

        if !errors.is_empty() {
            let mut message = String::from("invalid dotfiles.toml:");
            for e in &errors {
                let _ = write!(message, "\n  - {e}");
            }
            bail!(message);
        }
        Ok(warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
name = "my-hyprland"
wm   = "hyprland"

[packages]
pacman = ["hyprland", "waybar", "kitty"]
yay    = ["matugen-bin"]
"#;

    fn parse(text: &str) -> Manifest {
        toml::from_str(text).expect("parses")
    }

    #[test]
    fn minimal_manifest_defaults() {
        let m = parse(MINIMAL);
        assert_eq!(m.schema, 1);
        assert_eq!(m.version, "0.0.0");
        assert_eq!(m.distro, "arch");
        assert_eq!(m.mode, Mode::Symlink);
        assert_eq!(m.validate().unwrap(), Vec::<String>::new());
    }

    /// The M0 acceptance check: the example bundle survives read → write → read.
    #[test]
    fn example_bundle_round_trips() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("example");
        let m = Manifest::load(&dir).expect("example/dotfiles.toml loads");
        let again = parse(&m.to_toml().unwrap());
        assert_eq!(m, again);
        assert_eq!(m.wm, Wm::Hyprland);
        // Both component forms, from the real file.
        assert_eq!(m.components["shell"].pkg(), Some("fish"));
        assert_eq!(m.components["terminal"].pkg(), Some("kitty"));
        assert_eq!(m.components["cursor"].pkg(), None); // url-only, a manual step
    }

    #[test]
    fn hard_errors() {
        for bad in [
            "name = \"My Rice\"\nwm = \"hyprland\"",
            "name = \"x\"\nwm = \"hyprland\"\n[hooks]\npre_install = \"../evil.sh\"",
            "name = \"x\"\nwm = \"hyprland\"\n[hooks]\npre_install = \"/etc/evil.sh\"",
            "name = \"x\"\nwm = \"hyprland\"\n[[assets]]\nsrc = \"assets/w\"\ndest = \"Pictures\"",
        ] {
            assert!(parse(bad).validate().is_err(), "should reject: {bad}");
        }
        // Missing or unknown required fields never get as far as validate().
        assert!(toml::from_str::<Manifest>("wm = \"hyprland\"").is_err());
        assert!(toml::from_str::<Manifest>("name = \"x\"\nwm = \"bspwm\"").is_err());
        assert!(
            toml::from_str::<Manifest>(
                "name = \"x\"\nwm = \"sway\"\n[packages]\npacman = \"kitty\""
            )
            .is_err()
        );
    }

    #[test]
    fn warnings_do_not_block() {
        let m = parse(
            r#"
schema = 9
name   = "x"
wm     = "hyprland"
distro = "debian"

[packages]
pacman = ["kitty", "kitty"]

[components]
bar = { pkg = "waybar" }
"#,
        );
        let w = m.validate().expect("warnings only");
        assert_eq!(w.len(), 4, "{w:#?}");
    }
}

#[cfg(test)]
mod ignore_tests {
    use super::*;

    fn hit(path: &str) -> Option<&'static str> {
        // The user's own entries, on top of the default list.
        let patterns = [
            "config/hypr/settings.json".to_string(),
            "config/hypr/scripts/*.log".to_string(),
        ];
        ignored(&patterns, Path::new(path))
            .map(|p| Box::leak(p.to_string().into_boxed_str()) as &str)
    }

    #[test]
    fn default_and_written_patterns() {
        assert_eq!(
            hit("config/hypr/settings.json"),
            Some("config/hypr/settings.json")
        );
        assert_eq!(
            hit("config/hypr/scripts/debug.log"),
            Some("config/hypr/scripts/*.log")
        );
        assert_eq!(hit(".git/objects/ab/cdef"), Some(".git/"));
        assert_eq!(hit("preview/demo.mp4"), Some("*.mp4"));
        assert_eq!(hit("config/Code/User/settings.json"), Some("Code/"));
        assert_eq!(hit("config/x/History/index"), Some("*/History/"));
        assert_eq!(hit("config/x/Trust Tokens-journal"), Some("*Trust Tokens*"));

        // The screenshot the `preview` field points at is the reason preview/ exists.
        assert_eq!(hit("preview/screenshot.png"), None);
        assert_eq!(hit("config/hypr/hyprland.conf"), None);
        assert_eq!(hit("config/hypr/settings.jsonc"), None);
    }
}
