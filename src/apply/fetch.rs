//! Getting a bundle into the store. The only place a remote repo touches this machine.
//!
//! Nothing here fetches a `url` out of a manifest — that stays a manual step
//! (invariant 11). This clones the bundle itself and nothing else.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::bundle::Bundle;
use crate::paths;

/// Shallow, because real rice repos are 75 MB+ of history nobody is going to read. The
/// clone lands next to the store under a dot-name and is only moved into place once it
/// has turned out to be a bundle — a half-cloned directory must never become an entry.
pub fn clone(url: &str, branch: Option<&str>, as_name: Option<&str>) -> Result<String> {
    let store = paths::store();
    std::fs::create_dir_all(&store)?;
    let staging = store.join(".fetching");
    let _ = std::fs::remove_dir_all(&staging);

    let mut git = Command::new("git");
    git.args(["clone", "--depth", "1"]);
    if let Some(branch) = branch {
        git.args(["--branch", branch]);
    }
    git.arg(url).arg(&staging);
    let ok = git
        .status()
        .context("git not found — dotpack shells out to it rather than linking a git crate")?
        .success();
    if !ok {
        let _ = std::fs::remove_dir_all(&staging);
        bail!("git clone {url} failed");
    }

    adopt(&staging, as_name)
}

/// A repo with no `dotfiles.toml` is rejected here and the clone is thrown away. There is
/// deliberately no fallback that runs a foreign `install.sh` (invariant 10).
fn adopt(staging: &Path, as_name: Option<&str>) -> Result<String> {
    let named = || -> Result<String> {
        let bundle = Bundle::open(staging)?;
        bundle.manifest.validate()?;
        Ok(match as_name {
            // `--as` names a directory in the store, so it answers to the same rule the
            // manifest's own `name` does — otherwise `--as ../../evil` is a path escape.
            Some(given) if !crate::manifest::valid_name(given) => {
                bail!("`{given}` is not a valid bundle name ([a-z0-9._-]+)")
            }
            Some(given) => given.to_string(),
            None => bundle.manifest.name.clone(),
        })
    }();
    let name = match named {
        Ok(name) => name,
        Err(e) => {
            let _ = std::fs::remove_dir_all(staging);
            return Err(e);
        }
    };

    let entry = paths::store().join(&name);
    if entry.symlink_metadata().is_ok() {
        let _ = std::fs::remove_dir_all(staging);
        bail!("the store already has a bundle called `{name}` — `--as <other-name>`");
    }
    std::fs::rename(staging, &entry)?;
    Ok(name)
}
