//! Where a bundle comes from: `github:U/R`, a git URL, a local path, or a name already
//! in the store. Parsing only — cloning is `apply::fetch`'s job (invariant 1).
//!
//! Syntax: [profiles.md §2](../docs/profiles.md).

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::paths;

#[derive(Debug, PartialEq)]
pub enum Source {
    /// A name already in the store.
    Store(String),
    /// Used **where it is**, never copied: while working on your own repo, the file you
    /// edit has to stay the file in your repo.
    Local(PathBuf),
    Git {
        url: String,
        branch: Option<String>,
    },
}

pub fn parse(target: &str) -> Result<Source> {
    for (prefix, host) in [("github:", "github.com"), ("gitlab:", "gitlab.com")] {
        if let Some(rest) = target.strip_prefix(prefix) {
            let rest = variant(rest)?;
            let mut parts = rest.splitn(3, '/');
            let (Some(user), Some(repo)) = (parts.next(), parts.next()) else {
                bail!("`{target}` is not {prefix}user/repo");
            };
            if user.is_empty() || repo.is_empty() {
                bail!("`{target}` is not {prefix}user/repo");
            }
            return Ok(Source::Git {
                url: format!("https://{host}/{user}/{repo}.git"),
                branch: parts.next().filter(|b| !b.is_empty()).map(str::to_string),
            });
        }
    }

    if ["https://", "http://", "ssh://", "file://", "git@"]
        .iter()
        .any(|p| target.starts_with(p))
    {
        return Ok(Source::Git {
            url: variant(target)?.to_string(),
            branch: None,
        });
    }

    // A bare name is a store entry; anything with a separator in it, or that is simply
    // there, is a path.
    if target.contains('/') || target.starts_with('~') || Path::new(target).exists() {
        return Ok(Source::Local(paths::expand(target)));
    }
    Ok(Source::Store(target.to_string()))
}

/// `#catppuccin` — reserved so adding in-bundle variants later is not a breaking change
/// ([profiles.md §2](../docs/profiles.md)).
fn variant(rest: &str) -> Result<&str> {
    match rest.split_once('#') {
        Some((_, name)) => bail!("in-bundle variants (`#{name}`) are not supported in v1"),
        None => Ok(rest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(url: &str, branch: Option<&str>) -> Source {
        Source::Git {
            url: url.to_string(),
            branch: branch.map(str::to_string),
        }
    }

    #[test]
    fn the_table_in_profiles_md() {
        for (given, want) in [
            (
                "github:caelestia-dots/shell",
                git("https://github.com/caelestia-dots/shell.git", None),
            ),
            (
                "github:user/repo/dev",
                git("https://github.com/user/repo.git", Some("dev")),
            ),
            (
                "gitlab:user/repo",
                git("https://gitlab.com/user/repo.git", None),
            ),
            (
                "https://git.example.com/x.git",
                git("https://git.example.com/x.git", None),
            ),
            (
                "git@github.com:user/repo.git",
                git("git@github.com:user/repo.git", None),
            ),
            ("my-hyprland", Source::Store("my-hyprland".into())),
        ] {
            assert_eq!(parse(given).unwrap(), want, "{given}");
        }
        assert!(matches!(parse("./x").unwrap(), Source::Local(_)));
        assert!(parse("github:caelestia-dots/shell#catppuccin").is_err());
        assert!(parse("github:justauser").is_err());
    }
}
