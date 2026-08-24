//! `write_bundle()` — collect's output.
//!
//! It lives in `apply/` and not in a `collect.rs` for one reason: otherwise the
//! one-writer invariant would be false the first time a bundle is created. `collect` is a
//! scan that produces a plan; the plan is applied here.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::scan::Collected;

pub fn write_bundle(collected: &Collected, out: &Path, git: bool) -> Result<Vec<String>> {
    if out.is_dir() && out.read_dir()?.next().is_some() {
        bail!(
            "{} is not empty. Pass --out <dir> for somewhere else — or, if it is a chezmoi \
             or stow repo you already keep, `collect --external` (M5) adds a manifest to it \
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
