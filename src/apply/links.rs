//! Placing, removing and repointing links, and the bookkeeping of the directories that
//! had to be created to place them.

use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::backup;
use super::ledger::{self, Kind};
use crate::bundle;
use crate::paths;

/// What is actually at a link's target right now.
#[derive(Debug, PartialEq)]
pub enum State {
    /// A link into the active bundle, as recorded.
    Linked,
    /// An application deleted the link and wrote a real file in its place — the only
    /// thing `sync` exists for. A link pointing somewhere else entirely counts too.
    Detached,
    Missing,
}

pub fn state(entry: &ledger::Link, bundle_root: &Path) -> State {
    let target = paths::expand(&entry.target);
    match std::fs::read_link(&target) {
        Ok(dest) if dest.starts_with(bundle_root) => State::Linked,
        Ok(_) => State::Detached,
        Err(_) if target.symlink_metadata().is_err() => State::Missing,
        Err(_) => State::Detached,
    }
}

/// Place one link. `previous` is the ledger entry for the same target when the switch is
/// repointing a link we already own — its adopted backup and created directories carry
/// over, because they describe the target, not the bundle behind it.
pub fn place(
    link: &bundle::Link,
    previous: Option<&ledger::Link>,
    stamp: &str,
    notes: &mut Vec<String>,
) -> Result<ledger::Link> {
    let target = &link.target;

    let mkdirs = match previous {
        Some(p) => p.mkdirs.clone(),
        None => {
            let missing = missing_parents(target);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            missing.iter().map(|d| paths::contract(d)).collect()
        }
    };

    // Whatever is in the way: our own link goes, anything else is backed up first.
    let adopted = match target.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() && previous.is_some() => {
            std::fs::remove_file(target)?;
            previous.and_then(|p| p.adopted_backup.clone())
        }
        Ok(meta) => {
            let recorded = backup::adopt(target, stamp)?;
            if previous.is_some() {
                // The link we placed was replaced by a real file while the bundle was
                // active. The ledger keeps pointing at the *user's* original.
                notes.push(format!(
                    "{} was no longer our link — moved to backups/{recorded}",
                    entry_label(target, meta.file_type().is_dir())
                ));
                previous.and_then(|p| p.adopted_backup.clone())
            } else {
                Some(recorded)
            }
        }
        Err(_) => previous.and_then(|p| p.adopted_backup.clone()),
    };

    symlink(&link.source, target).with_context(|| format!("linking {}", target.display()))?;

    Ok(ledger::Link {
        target: paths::contract(target),
        kind: if link.dir { Kind::Dir } else { Kind::File },
        adopted_backup: adopted,
        mkdirs,
    })
}

/// Remove a link for good: the adopted original comes back and the directories we
/// created to place it go away if they are empty.
pub fn remove(entry: &ledger::Link, notes: &mut Vec<String>) -> Result<()> {
    let target = paths::expand(&entry.target);

    match target.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => std::fs::remove_file(&target)?,
        // Not ours any more. Deleting someone else's file to make room for a restore is
        // exactly the destruction invariant 2 exists to prevent.
        Ok(_) => {
            notes.push(format!(
                "{} is a real file now, not our link — left alone",
                entry.target
            ));
            return Ok(());
        }
        Err(_) => {}
    }

    if let Some(recorded) = &entry.adopted_backup
        && !backup::restore(recorded, &target)?
    {
        notes.push(format!(
            "backups/{recorded} is gone — {} could not be restored",
            entry.target
        ));
    }

    // Deepest first, and only while empty.
    let mut created: Vec<PathBuf> = entry.mkdirs.iter().map(|d| paths::expand(d)).collect();
    created.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for dir in created {
        let _ = std::fs::remove_dir(dir);
    }
    Ok(())
}

/// Which links go, and which are placed — the second carrying the ledger entry for the
/// same target when there is one, so a repoint keeps its backup and its mkdirs.
pub fn diff<'a>(
    old: &'a [ledger::Link],
    new: &'a [bundle::Link],
) -> (
    Vec<&'a ledger::Link>,
    Vec<(&'a bundle::Link, Option<&'a ledger::Link>)>,
) {
    let entry_for = |link: &bundle::Link| {
        let target = paths::contract(&link.target);
        old.iter().find(move |e| e.target == target)
    };
    let wanted: Vec<String> = new.iter().map(|l| paths::contract(&l.target)).collect();

    (
        old.iter().filter(|e| !wanted.contains(&e.target)).collect(),
        new.iter().map(|l| (l, entry_for(l))).collect(),
    )
}

/// The directories above `target` that do not exist yet, outermost first.
fn missing_parents(target: &Path) -> Vec<PathBuf> {
    let mut missing = Vec::new();
    let mut current = target.parent();
    while let Some(dir) = current {
        if dir.symlink_metadata().is_ok() {
            break;
        }
        missing.push(dir.to_path_buf());
        current = dir.parent();
    }
    missing.reverse();
    missing
}

fn entry_label(target: &Path, is_dir: bool) -> String {
    format!(
        "{} {}",
        if is_dir { "directory" } else { "file" },
        paths::contract(target)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(target: &str) -> ledger::Link {
        ledger::Link {
            target: target.into(),
            kind: Kind::Dir,
            adopted_backup: None,
            mkdirs: Vec::new(),
        }
    }

    #[test]
    fn diff_splits_by_target() {
        unsafe { std::env::set_var("HOME", "/tmp/dp-home") }
        let old = [entry("~/.config/hypr"), entry("~/.config/waybar")];
        let new = [
            bundle::Link {
                source: "/store/b/config/hypr".into(),
                target: "/tmp/dp-home/.config/hypr".into(),
                dir: true,
            },
            bundle::Link {
                source: "/store/b/config/kitty".into(),
                target: "/tmp/dp-home/.config/kitty".into(),
                dir: true,
            },
        ];
        let (removed, placed) = diff(&old, &new);
        assert_eq!(
            removed.iter().map(|e| &e.target).collect::<Vec<_>>(),
            ["~/.config/waybar"]
        );
        // hypr is a repoint and knows its old entry; kitty is new.
        assert!(placed[0].1.is_some());
        assert!(placed[1].1.is_none());
    }

    #[test]
    fn missing_parents_are_outermost_first() {
        let deep = Path::new("/tmp/dp-does-not-exist/a/b/c.conf");
        assert_eq!(
            missing_parents(deep),
            [
                PathBuf::from("/tmp/dp-does-not-exist"),
                PathBuf::from("/tmp/dp-does-not-exist/a"),
                PathBuf::from("/tmp/dp-does-not-exist/a/b"),
            ]
        );
        assert!(missing_parents(Path::new("/tmp/x.conf")).is_empty());
    }
}
