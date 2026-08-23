//! Reads and returns suggestions. **Nothing in here writes to disk** (invariant 1) — the
//! bundle a collect produces is written by `apply::write`.

pub mod deps;
pub mod fonts;
pub mod refs;
pub mod secrets;
pub mod wm;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use walkdir::WalkDir;

use crate::manifest::{Component, ComponentDetail, Manifest, Mode, Packages, Wm};
use crate::paths;
use crate::pkg;

/// A whole scan: the manifest it would write, the files it would copy, and everything
/// the user has to be told before either happens.
pub struct Collected {
    pub manifest: Manifest,
    /// Every package, with the line that suggested it. The wizard's checklist (M6) reads
    /// the same list; on the CLI it is what lets someone see *why* `texinfo` turned up
    /// and strike it out.
    pub suggestions: Vec<deps::Suggestion>,
    /// `(where it is now, where it goes in the bundle)`.
    pub files: Vec<(PathBuf, PathBuf)>,
    pub secrets: Vec<secrets::Finding>,
    pub dangling: Vec<refs::Reference>,
    pub warnings: Vec<String>,
}

/// Everything below is derived from one input — the set of selected directories — so it
/// all runs at once (design.md §4.1). Screen order is the TUI's business, not this one's.
pub fn collect(
    selection: &[String],
    ignore: &[String],
    name: Option<String>,
    wm: Wm,
) -> Result<Collected> {
    let mut warnings = Vec::new();
    let selection = match selection {
        [] => default_selection(wm, &mut warnings),
        chosen => chosen.to_vec(),
    };
    if selection.is_empty() {
        bail!("nothing to collect — name the directories under ~/.config to include");
    }

    // --- files ---
    let mut files = Vec::new();
    let mut sources = Vec::new();
    for directory in &selection {
        let source = paths::config().join(directory);
        if !source.is_dir() {
            warnings.push(format!("~/.config/{directory} does not exist"));
            continue;
        }
        // Walking into the active bundle through its own links would collect a bundle
        // into itself.
        if std::fs::canonicalize(&source).is_ok_and(|real| real.starts_with(paths::store())) {
            warnings.push(format!(
                "~/.config/{directory} belongs to the active bundle — already collected"
            ));
            continue;
        }
        for file in files_under(&source) {
            let relative = Path::new("config").join(directory).join(&file);
            if let Some(pattern) = crate::manifest::ignored(ignore, &relative) {
                if !DEFAULT_PATTERN_NOTED.contains(&pattern) {
                    warnings.push(format!("ignored {} ({pattern})", relative.display()));
                }
                continue;
            }
            let absolute = source.join(&file);
            if let Some(entry) = secrets::denied(&Path::new(".config").join(directory).join(&file))
            {
                warnings.push(format!(
                    "kept out: {} — {entry} is on the deny-list, and a bundle is shared",
                    paths::contract(&absolute)
                ));
                continue;
            }
            sources.push(absolute.clone());
            files.push((absolute, relative));
        }
    }

    // Checked against the source tree, never against a bundle: an ignored path is by
    // definition absent from a bundle, so there every correct pattern matches nothing.
    for pattern in ignore {
        if !matched_anything(pattern, &selection) {
            warnings.push(format!("ignore pattern `{pattern}` matches nothing here"));
        }
    }

    // --- the scans, all off the same file set ---
    let (suggestions, package_warnings) = deps::scan(&sources, wm);
    warnings.extend(package_warnings);
    let (found_fonts, font_warnings) = fonts::scan(&sources);
    warnings.extend(font_warnings);
    let dangling = refs::scan(&sources)
        .into_iter()
        .filter(refs::Reference::dangling)
        .collect();
    let secrets = secrets::scan(&sources);

    // --- the manifest ---
    let mut packages = Packages::default();
    let aur_field = matches!(pkg::helper().as_deref(), Some("paru"));
    for suggestion in &suggestions {
        let list = match (suggestion.aur, aur_field) {
            (false, _) => &mut packages.pacman,
            (true, true) => &mut packages.paru,
            (true, false) => &mut packages.yay,
        };
        list.push(suggestion.package.clone());
    }

    let mut components = std::collections::BTreeMap::new();
    components.insert("wm".to_string(), Component::Pkg(wm_name(wm).to_string()));
    for finding in &found_fonts {
        let mut detail = ComponentDetail {
            name: Some(finding.name.clone()),
            ..Default::default()
        };
        match &finding.source {
            fonts::Source::Package(package) => {
                detail.pkg = Some(package.clone());
                if !packages.pacman.contains(package) && !packages.yay.contains(package) {
                    packages.pacman.push(package.clone());
                }
            }
            fonts::Source::Ship(path) => {
                detail.path = Some(paths::contract(path));
                if !finding.role.starts_with("font") {
                    warnings.push(format!(
                        "{} is not provided by any package. Copy it into the bundle's \
                         local/share/ yourself, or leave it as a manual step",
                        paths::contract(path)
                    ));
                }
            }
            fonts::Source::Missing => {
                detail.note = Some("not installed on the machine this was collected on".into());
                warnings.push(format!(
                    "{} asks for `{}`, which is not installed here — fontconfig silently \
                     substitutes another",
                    finding.reason, finding.name
                ));
            }
        }
        components.insert(finding.role.to_string(), Component::Full(detail));
    }

    // A font nothing provides ships with the bundle: the receiver then needs no network
    // and no manual step. *The bundle is the download.*
    for finding in &found_fonts {
        if let fonts::Source::Ship(path) = &finding.source
            && finding.role.starts_with("font")
        {
            let directory = if path.is_dir() {
                path.clone()
            } else {
                path.parent().unwrap_or(path).to_path_buf()
            };
            let group = directory
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            for file in files_under(&directory) {
                files.push((
                    directory.join(&file),
                    Path::new("local/share/fonts").join(&group).join(&file),
                ));
            }
        }
    }

    packages.pacman.sort();
    packages.pacman.dedup();
    packages.yay.sort();
    packages.yay.dedup();
    packages.paru.sort();
    packages.paru.dedup();

    Ok(Collected {
        suggestions,
        manifest: Manifest {
            schema: crate::manifest::SCHEMA,
            name: name.unwrap_or_else(|| default_name(wm)),
            version: "0.0.0".into(),
            description: String::new(),
            author: std::env::var("USER").ok(),
            homepage: None,
            license: None,
            wm,
            distro: "arch".into(),
            preview: None,
            mode: Mode::Symlink,
            managed_by: None,
            services: Vec::new(),
            ignore: ignore.to_vec(),
            requires: Default::default(),
            packages,
            components,
            hooks: None,
            assets: Vec::new(),
        },
        files,
        secrets,
        dangling,
        warnings,
    })
}

/// Patterns from the default list are not worth a line each — nobody wonders why `.git`
/// was left out.
const DEFAULT_PATTERN_NOTED: &[&str] = &[".git/", "node_modules/", "*.log"];

/// The WM's own directory, plus the directories of what its config actually starts and
/// points at. This is the CLI's half of the wizard's "WM-related ones pre-ticked".
fn default_selection(wm: Wm, warnings: &mut Vec<String>) -> Vec<String> {
    let own = wm::rules(wm).dir.to_string();
    let files = files_under(&paths::config().join(&own))
        .into_iter()
        .map(|f| paths::config().join(&own).join(f))
        .collect::<Vec<_>>();
    if files.is_empty() {
        warnings.push(format!("~/.config/{own} is empty or missing"));
        return Vec::new();
    }

    let (suggestions, _) = deps::scan(&files, wm);
    let referenced: BTreeSet<String> = refs::scan(&files)
        .iter()
        .filter_map(|r| r.path.as_ref())
        .filter_map(|p| p.strip_prefix(paths::config()).ok())
        .filter_map(|p| p.components().next())
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();

    let mut selection = vec![own.clone()];
    let Ok(entries) = std::fs::read_dir(paths::config()) else {
        return selection;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == own || !entry.path().is_dir() {
            continue;
        }
        // `swayosd` is the directory and `swayosd-git` the package, so the AUR suffixes
        // come off — but only those. Prefix matching pre-ticks `~/.config/zen` for
        // `zen-browser`, and a browser profile is 2252 files of the user's own.
        let by_package = suggestions.iter().any(|s| {
            let package = s.package.trim_end_matches("-git").trim_end_matches("-bin");
            package == name || s.package == name
        });
        if by_package || referenced.contains(&name) {
            selection.push(name);
        }
    }
    selection.sort();
    selection
}

fn matched_anything(pattern: &str, selection: &[String]) -> bool {
    selection.iter().any(|directory| {
        files_under(&paths::config().join(directory))
            .iter()
            .any(|file| {
                let relative = Path::new("config").join(directory).join(file);
                crate::manifest::ignored(std::slice::from_ref(&pattern.to_string()), &relative)
                    .is_some()
            })
    })
}

fn files_under(directory: &Path) -> Vec<PathBuf> {
    WalkDir::new(directory)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| !e.file_type().is_dir())
        .filter_map(|e| e.path().strip_prefix(directory).ok().map(Path::to_path_buf))
        .collect()
}

fn wm_name(wm: Wm) -> &'static str {
    match wm {
        Wm::Hyprland => "hyprland",
        Wm::Sway => "sway",
        Wm::I3 => "i3",
    }
}

fn default_name(wm: Wm) -> String {
    let user = std::env::var("USER").unwrap_or_else(|_| "my".into());
    format!("{}-{}", user.to_ascii_lowercase(), wm_name(wm))
}
