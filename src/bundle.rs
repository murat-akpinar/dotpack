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

    /// Every file the bundle ships, for the scans that read it whole. `.git` is skipped:
    /// scanning a repo's object store is minutes of work and zero findings.
    pub fn files(&self) -> Vec<PathBuf> {
        WalkDir::new(&self.root)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|e| e.file_name() != ".git")
            .filter_map(Result::ok)
            .filter(|e| !e.file_type().is_dir())
            .map(|e| e.into_path())
            .collect()
    }

    /// Every shipped file paired with the path it will live at. Not the same list as
    /// [`Self::links`]: that one stops at the depth rule's directory, this one is per
    /// file, because a reference inside a file resolves against the file's own target.
    pub fn shipped(&self) -> Result<Vec<(PathBuf, PathBuf)>> {
        let mut out = Vec::new();
        // Same rule as [`Self::links`]: in `external` mode the tree belongs to
        // chezmoi/stow. Its files are not ours to place, so they are not ours to judge —
        // a `dot_config/` name resolves to nothing here and every reference in it would
        // be reported as dangling.
        if self.manifest.mode == Mode::External {
            return Ok(out);
        }
        for (area, base) in [
            ("config", paths::config()),
            ("home", paths::home()),
            ("local", paths::local()),
        ] {
            for rel in files_under(&self.root.join(area))? {
                out.push((self.root.join(area).join(&rel), base.join(&rel)));
            }
        }
        Ok(out)
    }

    /// `assets` are **copied**, never linked, so they are not part of `links()` — and
    /// they are copied on **every** activation, over nothing that is already there.
    ///
    /// That is the answer to TODO.md Phase 0's open question, and it is the one that
    /// needs no state: a switch never removes an asset (spec/manifest.md), so the second
    /// activation finds every file present and copies none of it. A `hooks_ran`-style
    /// ledger field would buy exactly one different case — a file the user deleted — and
    /// there, putting the bundle back is what re-running `use` means everywhere else in
    /// this tool.
    ///
    /// A directory `src` expands to its files, so the copy has one shape to handle and
    /// `dest` mirrors the tree. Empty in `external` mode, for [`Self::links`]' reason.
    pub fn assets(&self) -> Result<Vec<(PathBuf, PathBuf)>> {
        let mut out = Vec::new();
        if self.manifest.mode == Mode::External {
            return Ok(out);
        }
        for asset in &self.manifest.assets {
            let (source, dest) = (self.root.join(&asset.src), paths::expand(&asset.dest));
            if source.is_dir() {
                for rel in files_under(&source)? {
                    out.push((source.join(&rel), dest.join(&rel)));
                }
            } else {
                out.push((source, dest));
            }
        }
        Ok(out)
    }
}

// --- store view start ---

/// One line of `dotpack ls`, and one row of the TUI's main screen. Both need exactly the
/// same four questions answered, and answering them twice is how the two faces of a tool
/// start disagreeing about what is on the machine.
pub struct Row {
    pub name: String,
    pub path: PathBuf,
    /// `Err` carries the message shown in place of the columns: a bundle whose manifest
    /// does not parse still has to be listed, because the list is where you find out.
    pub bundle: std::result::Result<Bundle, String>,
    pub active: bool,
    /// Only asked of the active bundle — nothing else has anything on disk to detach, and
    /// scanning every bundle in the store for secrets on every `ls` is minutes of work.
    pub detached: usize,
    pub secrets: usize,
}

pub fn rows() -> Result<Vec<Row>> {
    let ledger = crate::apply::ledger::Ledger::load()?;
    let mut rows = Vec::new();
    for path in store_list()? {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let active = ledger.active.as_deref() == Some(&name);
        let bundle = Bundle::open(&path).map_err(|e| format!("{e}"));
        let (mut detached, mut secrets) = (0, 0);
        if let Ok(b) = &bundle
            && active
        {
            detached = ledger
                .links
                .iter()
                .filter(|e| {
                    crate::apply::links::state(e, &b.root) == crate::apply::links::State::Detached
                })
                .count();
            // In symlink mode the active bundle's files are the ones being edited, so a
            // token added the day after `collect` is seen by nothing unless this looks
            // too (design.md §6).
            secrets = crate::scan::secrets::scan(&b.files()).len();
        }
        rows.push(Row {
            name,
            path,
            bundle,
            active,
            detached,
            secrets,
        });
    }
    Ok(rows)
}

impl Bundle {
    /// The top-level names under `config/` — the detail panel's `config: hypr, fish, …`.
    pub fn config_dirs(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.root.join("config"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        names
    }
}

// --- store view end ---

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
                let path = entry?.path();
                // `.fetching` is a clone in progress, or one that died halfway.
                if !path
                    .file_name()
                    .is_some_and(|n| n.as_encoded_bytes()[0] == b'.')
                {
                    bundles.push(path);
                }
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
        // One directory link per top-level config dir — 400-odd files, not 400 links.
        assert_eq!(
            targets,
            [
                "/tmp/dp-home/.config/cava",
                "/tmp/dp-home/.config/hypr",
                "/tmp/dp-home/.config/kitty",
                "/tmp/dp-home/.config/matugen",
                "/tmp/dp-home/.config/swayosd"
            ]
        );
        assert!(links.iter().all(|l| l.dir));
        assert_eq!(links[1].source, root.join("config/hypr"));
    }

    #[test]
    fn escaping_paths_rejected() {
        assert!(safe_rel("hooks/post-install.sh"));
        assert!(!safe_rel("../evil.sh"));
        assert!(!safe_rel("/etc/evil.sh"));
        assert!(!safe_rel("hooks/../../evil.sh"));
    }
}
