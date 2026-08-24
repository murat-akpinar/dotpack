//! Reference integrity: every shipped text file is read for the files it points at, and
//! every one of those is resolved. A bundle that ships `kitty.conf` without the
//! `catppuccin.conf` it includes installs a kitty that errors on every start.
//!
//! Reads only. design.md §5.1.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::paths;

/// Catches bare relative paths, which have no other marker: `include catppuccin.conf`.
/// It does not have to be complete — the token extractor below consults none of it.
const KEYWORDS: &[&str] = &["source", "include", "@import", "require", "dofile"];

/// Runtime paths, not bundle content. `QS_CACHE_DIR="$HOME/.cache/quickshell"` is a
/// directory the script creates, not a file somebody forgot to ship — and a check that
/// cries wolf on `~/.cache` gets switched off, taking the real findings with it.
const RUNTIME: &[&str] = &[".cache/", ".local/state/", ".local/share/"];

/// The area roots themselves. `du -sh ~/.config ~/.cache` names three directories; none
/// of them is a file anybody forgot to ship.
const AREA_ROOTS: &[&str] = &[
    "",
    ".config",
    ".local",
    ".local/share",
    ".local/bin",
    ".cache",
];

/// ...except the four `~/.local/share` directories the bundle format actually maps.
/// A font under `local/share/fonts/` is content; `~/.local/share/focustime` is a
/// directory a script creates at runtime.
const LOCAL_SHARE_CONTENT: &[&str] = &[
    ".local/share/fonts",
    ".local/share/icons",
    ".local/share/themes",
    ".local/share/applications",
];

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    /// The bundle already ships it.
    Shipped,
    /// It exists here but is not selected — the common miss, and the one worth offering.
    Addable,
    /// `/usr/share/…`: it belongs to a package, so it goes to the dependency scan.
    SystemPath,
    /// Nothing is there. The reference is already dead on this machine.
    Dead,
    /// Contains a `$` that is not `$HOME` — guessing is worse than saying so.
    Unresolved,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reference {
    pub from: PathBuf,
    pub line: usize,
    pub raw: String,
    pub path: Option<PathBuf>,
    pub verdict: Verdict,
}

impl Reference {
    /// The ones worth showing: everything the bundle would install broken.
    pub fn dangling(&self) -> bool {
        matches!(
            self.verdict,
            Verdict::Addable | Verdict::Dead | Verdict::Unresolved
        )
    }
}

/// `files` are the paths the selection will ship, at the place they live on this machine.
pub fn scan(files: &[PathBuf]) -> Vec<Reference> {
    let pairs: Vec<(PathBuf, PathBuf)> = files.iter().map(|f| (f.clone(), f.clone())).collect();
    scan_at(&pairs)
}

/// `(read from, will live at)`. For a bundle the two differ — the file sits in the store
/// and every reference inside it resolves against where it is going to be installed. That
/// is the whole validation-time check on somebody else's bundle: the same scan, run
/// before the links are placed rather than after.
pub fn scan_at(files: &[(PathBuf, PathBuf)]) -> Vec<Reference> {
    let shipped: BTreeSet<&Path> = files.iter().map(|(_, at)| at.as_path()).collect();
    let mut found = Vec::new();

    for (file, at) in files {
        // A bundle's README and manifest *describe* paths, they do not consume them.
        if matches!(
            file.file_name().and_then(|n| n.to_str()),
            Some("README.md") | Some("dotfiles.toml")
        ) {
            continue;
        }
        let Some(text) = read_text(file) else {
            continue;
        };
        let dir = at.parent().unwrap_or(Path::new("/"));

        for (index, line) in text.lines().enumerate() {
            for raw in references_in(line, dir) {
                // A bare variable is not a file reference — `$CACHE_FILE` is unknowable
                // and saying so on every line is how a check gets switched off. One with
                // a path in it (`$SCRIPT_DIR/../caching.sh`) is still worth reporting.
                if raw.contains('$') && !raw.contains('/') {
                    continue;
                }
                let path = resolve(&raw, dir);
                found.push(Reference {
                    verdict: verdict(&path, &shipped),
                    from: file.clone(),
                    line: index + 1,
                    raw,
                    path,
                });
            }
        }
    }
    found
}

// --- extraction start ---

/// The two extractors, in order. The second finds most of them.
fn references_in(line: &str, dir: &Path) -> Vec<String> {
    let line = substitute_dirname(line, dir);
    let trimmed = line.trim();
    if trimmed.starts_with('#') || trimmed.starts_with("//") {
        return Vec::new();
    }

    let mut found = Vec::new();

    // 1. keyword, taking the rest of the line as the reference.
    if let Some((keyword, rest)) = trimmed.split_once(|c: char| c.is_whitespace() || c == '=')
        && KEYWORDS.contains(&keyword.trim_end_matches('=').trim())
    {
        let keyword = keyword.trim_end_matches('=').trim();
        let value = clean(rest.trim().trim_start_matches('=').trim());
        // `require("nvchad.options")` names a Lua module, not a file — and neovim's
        // config is nothing but those. Only a value with a path in it is a reference.
        let module = matches!(keyword, "require" | "dofile") && !value.contains('/');
        // A value with a `$(` in it is the substitution's business, not this one's:
        // taking the rest of the line here would grab `$(dirname ` as a filename.
        if !value.is_empty() && !module && !value.contains("$(") {
            found.push(value);
        }
    }

    // 2. any token beginning `~/` or `$HOME/`, anywhere on the line. The paths that sit
    //    in ordinary argument position have no directive to look for.
    for token in line.split_whitespace() {
        for marker in ["~/", "$HOME/"] {
            if let Some(at) = token.find(marker) {
                let value = clean(&token[at..]);
                if !value.is_empty() && !found.contains(&value) {
                    found.push(value);
                }
            }
        }
    }
    found
}

/// `$(dirname "${BASH_SOURCE[0]}")` and `$(dirname "$0")` both mean "the directory of
/// this file". That one substitution is what turns a rice's own scripts from unreadable
/// into checkable.
fn substitute_dirname(line: &str, dir: &Path) -> String {
    let mut out = line.to_string();
    while let Some(start) = out.find("$(dirname") {
        let Some(end) = out[start..].find(')') else {
            break;
        };
        out.replace_range(start..start + end + 1, &dir.display().to_string());
    }
    out
}

/// Strip what surrounds a path in a config file, never what is in it. A reference in a
/// QML array or a JSON object ends `…/reload.sh"}`, and one character of leftover
/// punctuation turns a file that exists into a "dead reference".
fn clean(value: &str) -> String {
    value
        .trim()
        .trim_start_matches(|c| "\"'`([{".contains(c))
        // Cut, not trim: a sed expression ends `…/scripts|g` and a QML array
        // `…/init.sh"]`, and everything from the first of these on is not the path.
        .split(|c| "\"'`|,;)]}\\".contains(c))
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

// --- extraction end ---

fn resolve(raw: &str, dir: &Path) -> Option<PathBuf> {
    let expanded = if let Some(rest) = raw.strip_prefix("~/") {
        paths::home().join(rest)
    } else if let Some(rest) = raw.strip_prefix("$HOME/") {
        paths::home().join(rest)
    } else if raw.starts_with('/') {
        PathBuf::from(raw)
    } else {
        dir.join(raw)
    };
    // Anything else with a variable in it is reported, not guessed.
    if expanded.to_string_lossy().contains('$') {
        return None;
    }
    Some(normalize(&expanded))
}

/// Resolve `..` lexically. `$(dirname …)/../../caching.sh` is a real reference to a real
/// file, and comparing it against the selection unnormalized always says "not shipped".
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn verdict(path: &Option<PathBuf>, shipped: &BTreeSet<&Path>) -> Verdict {
    let Some(path) = path else {
        return Verdict::Unresolved;
    };
    // A directory counts as shipped when its contents are: `exec-once = quickshell -p
    // ~/.config/hypr/scripts/quickshell` names a directory, and every file in it is in
    // the selection.
    if shipped.contains(path.as_path()) || shipped.iter().any(|f| f.starts_with(path)) {
        return Verdict::Shipped;
    }
    if !path.starts_with(paths::home()) {
        return Verdict::SystemPath;
    }
    // Runtime paths are not bundle content, whether they exist yet or not.
    let below_home = path.strip_prefix(paths::home()).unwrap_or(path);
    let below_home = below_home.to_string_lossy();
    if AREA_ROOTS.contains(&below_home.as_ref()) {
        return Verdict::Shipped;
    }
    let content = LOCAL_SHARE_CONTENT.iter().any(|c| under(&below_home, c));
    if !content && (RUNTIME.iter().any(|r| under(&below_home, r)) || !below_home.starts_with('.')) {
        return Verdict::Shipped;
    }
    if path.symlink_metadata().is_ok() {
        Verdict::Addable
    } else {
        Verdict::Dead
    }
}

/// Whole components only, so `.cache` does not also mean `.cachalot`.
fn under(path: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

/// Text only — a font or a screenshot has no references in it, and reading one as UTF-8
/// is how a scan turns into noise.
pub(crate) fn read_text(file: &Path) -> Option<String> {
    let bytes = std::fs::read(file).ok()?;
    if bytes.iter().take(1024).any(|b| *b == 0) {
        return None;
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(line: &str) -> Vec<String> {
        references_in(line, Path::new("/here"))
    }

    #[test]
    fn keyword_extractor_catches_bare_relative_paths() {
        assert_eq!(
            extract("include ~/.config/kitty/catppuccin.conf"),
            ["~/.config/kitty/catppuccin.conf"]
        );
        assert_eq!(extract("include catppuccin.conf"), ["catppuccin.conf"]);
        assert_eq!(
            extract("source = ~/.config/hypr/config/env.conf"),
            ["~/.config/hypr/config/env.conf"]
        );
        assert_eq!(extract("@import \"colors.css\";"), ["colors.css"]);
        assert!(extract("# source = commented.conf").is_empty());
        assert!(extract("resource_type = ram").is_empty());
    }

    /// The one that finds most of them: paths in ordinary argument position, with no
    /// directive anywhere on the line.
    #[test]
    fn token_extractor_catches_argument_position() {
        assert_eq!(
            extract("exec-once = swayosd-server --style \"$HOME/.config/swayosd/style.css\""),
            ["$HOME/.config/swayosd/style.css"]
        );
        assert_eq!(
            extract("exec-once = quickshell -p ~/.config/hypr/scripts/quickshell/Shell.qml"),
            ["~/.config/hypr/scripts/quickshell/Shell.qml"]
        );
        assert_eq!(
            extract("SCRIPTS_DIR=\"$HOME/.config/hypr/scripts/quickshell\""),
            ["$HOME/.config/hypr/scripts/quickshell"]
        );
    }

    #[test]
    fn dirname_is_substituted_not_guessed() {
        assert_eq!(
            references_in(
                "RELOAD=\"$(dirname \"${BASH_SOURCE[0]}\")/quickshell/x.sh\"",
                Path::new("/scripts")
            ),
            Vec::<String>::new(),
            "not a ~ or $HOME token, so nothing is extracted — but the substitution ran"
        );
        assert_eq!(
            substitute_dirname(
                "source \"$(dirname \"$0\")/caching.sh\"",
                Path::new("/scripts")
            ),
            "source \"/scripts/caching.sh\""
        );
        // ...and with the substitution done, the keyword extractor resolves it.
        assert_eq!(
            references_in(
                "source \"$(dirname \"$0\")/caching.sh\"",
                Path::new("/scripts")
            ),
            ["/scripts/caching.sh"]
        );
    }

    #[test]
    fn runtime_paths_are_not_findings() {
        unsafe { std::env::set_var("HOME", "/tmp/dp-home") }
        let shipped = BTreeSet::new();
        let v = |raw: &str| verdict(&resolve(raw, Path::new("/here")), &shipped);
        assert_eq!(
            v("$HOME/.cache/quickshell"),
            Verdict::Shipped,
            "a directory the script creates"
        );
        assert_eq!(v("~/.local/state/x.log"), Verdict::Shipped);
        assert_eq!(
            v("~/Pictures/wallpaper.png"),
            Verdict::Shipped,
            "the user's, not the bundle's"
        );
        assert_eq!(
            v("/usr/share/themes/x"),
            Verdict::SystemPath,
            "belongs to a package"
        );
        assert_eq!(v("~/.config/hypr/gone.conf"), Verdict::Dead);
        assert_eq!(v("$XDG_RUNTIME_DIR/x"), Verdict::Unresolved);
    }

    /// Over the example bundle, which is a real rice: the numbers the design claims.
    #[test]
    fn example_bundle_reference_count() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("example/config");
        let mut by_keyword = 0;
        let mut by_token = 0;
        for entry in walkdir::WalkDir::new(&root).sort_by_file_name() {
            let entry = entry.unwrap();
            if entry.file_type().is_dir() {
                continue;
            }
            let Some(text) = read_text(entry.path()) else {
                continue;
            };
            let dir = entry.path().parent().unwrap();
            for line in text.lines() {
                for raw in references_in(line, dir) {
                    if raw.starts_with('~') || raw.starts_with('$') {
                        by_token += 1;
                    } else {
                        by_keyword += 1;
                    }
                }
            }
        }
        assert!(
            by_token > by_keyword,
            "{by_token} in argument position vs {by_keyword} by keyword"
        );
        println!("keyword: {by_keyword}, token: {by_token}");
    }
}
