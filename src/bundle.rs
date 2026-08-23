//! The bundle directory layout: where a file lands, and at what granularity it is linked.
//!
//! Reads only. Placing the links is `apply::links`' job.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use anyhow::Result;
use walkdir::WalkDir;

use crate::manifest::{Manifest, Mode};
use crate::paths;

/// One link to place: `source` (inside the bundle) → `target` (under `$HOME`).
/// (Prose here avoids the literal `sym`+`link` — invariant 1's grep must print nothing.)
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Link {
    pub source: PathBuf,
    pub target: PathBuf,
    /// `true` for a directory link — `config/` only.
    pub dir: bool,
}

pub struct Bundle {
    pub root: PathBuf,
    pub manifest: Manifest,
}

impl Bundle {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let manifest = Manifest::load(&root)?;
        Ok(Self { root, manifest })
    }

    /// Every link this bundle would place. Empty in `external` mode — there the files
    /// belong to chezmoi/stow and we touch none of them.
    pub fn links(&self) -> Result<Vec<Link>> {
        if self.manifest.mode == Mode::External {
            return Ok(Vec::new());
        }
        links(&self.root)
    }

    /// `assets` are **copied**, never linked, so they are not part of `links()`.
    ///
    /// ponytail: no caller yet. design.md §4.2's sequence has no step that copies them
    /// and no bundle in the repo has any, so *when* they are copied — first activation
    /// only, or every switch — is an open question in TODO.md Phase 0.
    #[allow(dead_code)]
    pub fn assets(&self) -> Vec<(PathBuf, PathBuf)> {
        self.manifest
            .assets
            .iter()
            .map(|a| (self.root.join(&a.src), paths::expand(&a.dest)))
            .collect()
    }
}

// --- layout start ---

pub fn links(root: &Path) -> Result<Vec<Link>> {
    let mut out = Vec::new();

    // config/ → directory link, at the depth rule's depth.
    for (rel, dir) in link_roots(&files_under(&root.join("config"))?) {
        out.push(Link {
            source: root.join("config").join(&rel),
            target: paths::config().join(&rel),
            dir,
        });
    }

    // home/ and local/ → per file, always. Both are *mixed* directories: a directory
    // link there would hide the user's own things (hand-installed fonts, ~/.local/bin).
    for (area, base) in [("home", paths::home()), ("local", paths::local())] {
        for rel in files_under(&root.join(area))? {
            out.push(Link {
                source: root.join(area).join(&rel),
                target: base.join(&rel),
                dir: false,
            });
        }
    }

    Ok(out)
}

/// Relative paths of every file below `dir`, sorted. A missing directory is not an error.
fn files_under(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir).sort_by_file_name().min_depth(1) {
        let entry = match entry {
            Ok(e) => e,
            // The bundle may simply not have this area.
            Err(e) if e.io_error().map(|io| io.kind()) == Some(std::io::ErrorKind::NotFound) => {
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        if !entry.file_type().is_dir() {
            files.push(entry.path().strip_prefix(dir)?.to_path_buf());
        }
    }
    Ok(files)
}

/// The link depth rule, over `config/`'s file list.
///
/// > Walk down from `config/`. If a directory contains files directly, link it and stop.
/// > If it only contains directories, descend again.
///
/// A file sitting directly in `config/` is linked as a file — the rule must never
/// promote `~/.config` itself into a single directory link.
fn link_roots(files: &[PathBuf]) -> BTreeSet<(PathBuf, bool)> {
    let dirs: BTreeSet<&Path> = files.iter().filter_map(|f| f.parent()).collect();
    let mut roots = BTreeSet::new();

    for file in files {
        let Some(parent) = file.parent().filter(|p| !p.as_os_str().is_empty()) else {
            roots.insert((file.clone(), false));
            continue;
        };
        let mut root = PathBuf::new();
        for component in parent.components() {
            root.push(component);
            // The shallowest directory below config/ that holds a file directly.
            if dirs.contains(root.as_path()) {
                break;
            }
        }
        roots.insert((root, true));
    }
    roots
}

/// Does this manifest-declared path stay inside the bundle? Hooks and `assets[].src`
/// come from someone else's repo.
pub fn safe_rel(p: &str) -> bool {
    let p = Path::new(p);
    p.components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// Every bundle directory in the store, sorted. Local-path bundles are links in there
/// and read back exactly like any other directory.
pub fn store_list() -> Result<Vec<PathBuf>> {
    let mut bundles = Vec::new();
    match std::fs::read_dir(paths::store()) {
        Ok(entries) => {
            for entry in entries {
                bundles.push(entry?.path());
            }
        }
        // Nothing has ever been added.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    bundles.sort();
    Ok(bundles)
}

// --- layout end ---

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(files: &[&str]) -> Vec<(String, bool)> {
        link_roots(&files.iter().map(PathBuf::from).collect::<Vec<_>>())
            .into_iter()
            .map(|(p, d)| (p.display().to_string(), d))
            .collect()
    }

    #[test]
    fn depth_rule() {
        // A directory holding files directly is the link.
        assert_eq!(
            roots(&[
                "hypr/hyprland.conf",
                "hypr/scripts/init.sh",
                "kitty/kitty.conf"
            ]),
            [("hypr".into(), true), ("kitty".into(), true)]
        );
        // Nothing above it → descend. This is the rice-installs-alongside case.
        assert_eq!(
            roots(&["hypr/themes/cyberpunk/theme.conf"]),
            [("hypr/themes/cyberpunk".into(), true)]
        );
        // ...and both at once: the deep theme still collapses into its own hypr link.
        assert_eq!(
            roots(&["hypr/hyprland.conf", "hypr/themes/x/theme.conf"]),
            [("hypr".into(), true)]
        );
        // A loose file in config/ is a file link, never a link over ~/.config itself.
        assert_eq!(roots(&["mimeapps.list"]), [("mimeapps.list".into(), false)]);
    }

    #[test]
    fn example_bundle_links() {
        unsafe { std::env::set_var("HOME", "/tmp/dp-home") }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("example");
        let links = links(&root).unwrap();
        let targets: Vec<String> = links
            .iter()
            .map(|l| l.target.display().to_string())
            .collect();
        // Three directory links, one per top-level config dir — 34 files, not 34 links.
        assert_eq!(
            targets,
            [
                "/tmp/dp-home/.config/hypr",
                "/tmp/dp-home/.config/kitty",
                "/tmp/dp-home/.config/swayosd"
            ]
        );
        assert!(links.iter().all(|l| l.dir));
        assert_eq!(links[0].source, root.join("config/hypr"));
    }

    #[test]
    fn escaping_paths_rejected() {
        assert!(safe_rel("hooks/post-install.sh"));
        assert!(!safe_rel("../evil.sh"));
        assert!(!safe_rel("/etc/evil.sh"));
        assert!(!safe_rel("hooks/../../evil.sh"));
    }
}
