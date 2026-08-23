//! Nothing is destroyed without a backup (invariant 2).
//!
//! A real file at a target path is *adopted*: moved under
//! `~/.local/state/dotpack/backups/<stamp>/` with its path below `$HOME` mirrored, and
//! recorded in the ledger. When the link that displaced it is removed for good, it comes
//! back.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::paths;

/// `2026-08-23T14-02-11` — the same instant as the ledger's `activated_at`, with the
/// colons taken out because it becomes a directory name.
pub fn stamp() -> String {
    super::ledger::now().replace(':', "-").replace('Z', "")
}

/// Move whatever is really at `target` into the backups. Returns the value the ledger
/// records: `<stamp>/<path below HOME>`.
pub fn adopt(target: &Path, stamp: &str) -> Result<String> {
    let relative = below_home(target);
    let dest = paths::backups().join(stamp).join(&relative);
    std::fs::create_dir_all(dest.parent().expect("backup path has a parent"))?;
    move_path(target, &dest)
        .with_context(|| format!("backing up {} to {}", target.display(), dest.display()))?;
    Ok(format!("{stamp}/{}", relative.display()))
}

/// Put an adopted file back. `false` means the ledger pointed at a backup that is no
/// longer there — worth reporting, not worth aborting a switch over.
pub fn restore(recorded: &str, target: &Path) -> Result<bool> {
    let source = paths::backups().join(recorded);
    if !source.symlink_metadata().is_ok() {
        return Ok(false);
    }
    std::fs::create_dir_all(target.parent().expect("target has a parent"))?;
    move_path(&source, target)
        .with_context(|| format!("restoring {} to {}", source.display(), target.display()))?;
    Ok(true)
}

/// `~/.config/hypr` → `.config/hypr`. Paths outside HOME keep their shape minus the
/// leading slash, so two backups can never collide.
fn below_home(target: &Path) -> PathBuf {
    match target.strip_prefix(paths::home()) {
        Ok(rest) => rest.to_path_buf(),
        Err(_) => PathBuf::from(target.to_string_lossy().trim_start_matches('/').to_string()),
    }
}

/// `rename` is atomic and is what this almost always is. It fails across filesystems —
/// a separately mounted `~/.config` is unusual but real — and `mv` is the fallback that
/// already knows how to copy a directory tree and delete the original.
fn move_path(from: &Path, to: &Path) -> Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    let status = std::process::Command::new("mv")
        .arg(from)
        .arg(to)
        .status()?;
    anyhow::ensure!(status.success(), "mv {} {}", from.display(), to.display());
    Ok(())
}
