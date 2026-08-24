//! `write_bundle()` — collect's output — and `copy_assets()`, the one thing a switch
//! copies instead of linking.
//!
//! It lives in `apply/` and not in a `collect.rs` for one reason: otherwise the
//! one-writer invariant would be false the first time a bundle is created. `collect` is a
//! scan that produces a plan; the plan is applied here.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::scan::Collected;

pub fn write_bundle(collected: &Collected, out: &Path, git: bool) -> Result<Vec<String>> {
    // `external`: the directory is someone's chezmoi or stow repo, already full and
    // already a git repo. One file is added to it — not a README on top of the one they
    // have, and not a `git init` in a tree that is already tracked.
    if collected.manifest.mode == crate::manifest::Mode::External {
        let file = out.join("dotfiles.toml");
        if file.exists() {
            bail!("{} already has a dotfiles.toml", out.display());
        }
        std::fs::create_dir_all(out)?;
        std::fs::write(file, collected.manifest.to_toml()?)?;
        return Ok(vec![format!(
            "no files copied — {} places them, `dotpack use` installs the packages",
            collected
                .manifest
                .managed_by
                .as_deref()
                .unwrap_or("your own tool")
        )]);
    }
    if out.is_dir() && out.read_dir()?.next().is_some() {
        bail!(
            "{} is not empty. Pass --out <dir> for somewhere else — or, if it is a chezmoi \
             or stow repo you already keep, `collect --external` adds a manifest to it \
             instead of a bundle beside it",
            out.display()
        );
    }
    std::fs::create_dir_all(out)?;

    for (source, relative) in &collected.files {
        let destination = out.join(relative);
        std::fs::create_dir_all(destination.parent().expect("a file has a parent"))?;
        // `fs::copy` preserves the mode bits (invariant 9). A rice ships dozens of
        // executable scripts and a read+write loop breaks every one of them silently.
        std::fs::copy(source, &destination)
            .with_context(|| format!("copying {}", source.display()))?;
    }

    std::fs::write(out.join("dotfiles.toml"), collected.manifest.to_toml()?)?;
    std::fs::write(
        out.join("README.md"),
        crate::post::readme(&collected.manifest),
    )?;

    let mut notes = Vec::new();
    if git && !out.join(".git").exists() && !init_repo(out) {
        notes.push("git init did not run — the bundle is a plain directory".to_string());
    }
    Ok(notes)
}

/// `sync`'s write-back: the file an application left in place of our link becomes the
/// bundle's version of that file.
///
/// This is **the only path by which a file enters a bundle after `collect`**, so the §6
/// content scan runs on it here. In symlink mode the bundle is a git repo that is
/// probably public; a token that arrives this way would be seen by nothing.
pub fn write_back(detached: &Path, into: &Path) -> Result<Vec<String>> {
    let files: Vec<PathBuf> = if detached.is_dir() {
        walkdir::WalkDir::new(detached)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| !e.file_type().is_dir())
            .map(|e| e.into_path())
            .collect()
    } else {
        vec![detached.to_path_buf()]
    };

    let findings = crate::scan::secrets::scan(&files);
    if !findings.is_empty() {
        let mut refused: Vec<String> = findings
            .iter()
            .map(|f| format!("{}:{} {}", f.file.display(), f.line, f.what))
            .collect();
        refused.dedup();
        return Ok(refused);
    }

    for file in files {
        // A file link writes to `into` itself; `into.join("")` would give it a trailing
        // slash, and copying onto that is EISDIR.
        let destination = match detached.is_dir() {
            true => into.join(file.strip_prefix(detached).unwrap_or(Path::new(""))),
            false => into.to_path_buf(),
        };
        std::fs::create_dir_all(destination.parent().expect("a file has a parent"))?;
        std::fs::copy(&file, &destination)
            .with_context(|| format!("writing {} back into the bundle", file.display()))?;
    }
    Ok(Vec::new())
}

/// `assets` land wherever the manifest says, and they are **copied, never linked** —
/// `dest` is a directory the user owns (`~/Pictures/wallpapers`), and adopting it into a
/// bundle would be an unpleasant surprise. For the same reason an existing file is never
/// overwritten: it is reported and left where it is (spec/manifest.md).
pub fn copy_assets(assets: &[(PathBuf, PathBuf)], notes: &mut Vec<String>) -> Result<usize> {
    let mut copied = 0;
    for (source, dest) in assets {
        if dest.exists() {
            notes.push(format!(
                "{} is already there — the bundle's copy was not written over it",
                crate::paths::contract(dest)
            ));
            continue;
        }
        std::fs::create_dir_all(dest.parent().expect("a file has a parent"))?;
        // `fs::copy` for the mode bits, as everywhere else (invariant 9).
        std::fs::copy(source, dest).with_context(|| format!("copying {}", source.display()))?;
        copied += 1;
    }
    Ok(copied)
}

/// The tool does not wrap git: this is the first commit and nothing else. Remotes and
/// pushing are the user's.
fn init_repo(out: &Path) -> bool {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(out)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    git(&["init", "--quiet"])
        && git(&["add", "-A"])
        && git(&["commit", "--quiet", "-m", "initial bundle"])
}
