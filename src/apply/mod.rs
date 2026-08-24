//! **The only module that writes to disk.** Everything else reads and proposes.
//!
//! This file holds the sequences and nothing else, so that design.md §4.2's thirteen
//! steps and profiles.md §3's ten-step switch stay readable as the code that runs them.

pub mod backup;
pub mod fetch;
pub mod ledger;
pub mod links;
pub mod system;
pub mod write;

use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::bundle::Bundle;
use crate::manifest::Component;
use crate::paths;
use crate::pkg;

use ledger::Ledger;

/// What a switch is about to do. Shown and confirmed before anything happens —
/// `enter` is never destructive (invariant 7).
pub struct Plan {
    pub name: String,
    pub packages: pkg::Plan,
    pub place: Vec<String>,
    pub remove: Vec<String>,
    pub detached: Vec<String>,
    pub warnings: Vec<String>,
    /// `components` carrying a `url`: the user installs these by hand. Never fetched
    /// (invariant 11).
    pub manual: Vec<String>,
    /// The hooks that are actually going to run, in the order they run, with their source
    /// — a script from someone else's repo is shown before it is approved (invariant 5).
    pub hooks: Vec<Hook>,
}

pub struct Hook {
    /// `pre_install` (before the packages) or `post_install` (after links and services).
    pub when: &'static str,
    pub path: String,
    pub script: String,
}

/// The flags that change what a switch does rather than what it switches to.
#[derive(Clone, Copy, Default)]
pub struct Options {
    pub no_hooks: bool,
    /// Run them even though the ledger says they already have.
    pub run_hooks: bool,
}

#[derive(Default)]
pub struct Summary {
    pub linked: usize,
    pub unlinked: usize,
    pub packages_failed: Vec<String>,
    pub notes: Vec<String>,
    pub backup_dir: Option<PathBuf>,
}

/// Everything the plan screen needs, and nothing written yet.
pub fn plan(bundle: &Bundle, options: Options) -> Result<Plan> {
    let ledger = Ledger::load()?;
    let new = bundle.links()?;
    let (remove, place) = links::diff(&ledger.links, &new);

    let active_root = ledger.active.as_ref().map(|name| paths::store().join(name));
    let detached = match &active_root {
        Some(root) => ledger
            .links
            .iter()
            .filter(|e| links::state(e, root) == links::State::Detached)
            .map(|e| e.target.clone())
            .collect(),
        None => Vec::new(),
    };

    let mut warnings = bundle.manifest.validate()?;
    warnings.extend(machine_warnings(bundle)?);
    let (hooks, hook_warning) = hooks(bundle, &ledger, options);
    warnings.extend(hook_warning);

    Ok(Plan {
        name: bundle.manifest.name.clone(),
        packages: pkg::plan(&bundle.manifest)?,
        place: place
            .iter()
            .map(|(l, _)| paths::contract(&l.target))
            .collect(),
        remove: remove.iter().map(|e| e.target.clone()).collect(),
        detached,
        warnings,
        manual: manual_steps(bundle),
        hooks,
    })
}

/// What only this machine can say about a foreign bundle. None of it blocks: a sway
/// config on hyprland, or a rice wanting a newer Hyprland, is still the user's call
/// ([manifest.md](../docs/manifest.md)).
fn machine_warnings(bundle: &Bundle) -> Result<Vec<String>> {
    let mut warnings = Vec::new();

    if let Some(running) = crate::scan::wm::detect()
        && running != bundle.manifest.wm
    {
        warnings.push(format!(
            "this bundle is for {} and {} is running — the files are placed, nothing reloads",
            name(bundle.manifest.wm),
            name(running)
        ));
    }
    warnings.extend(pkg::requires_warnings(&bundle.manifest.requires));

    // The reference check the author got at collect time, run again on the receiving
    // side: shipping `kitty.conf` without the `catppuccin.conf` it includes installs a
    // kitty that errors on every start (design.md §5.1).
    let shipped = bundle.shipped()?;
    for reference in crate::scan::refs::scan_at(&shipped) {
        if !reference.dangling() {
            continue;
        }
        warnings.push(format!(
            "{}:{} points at {}, which the bundle does not ship",
            reference
                .from
                .strip_prefix(&bundle.root)
                .unwrap_or(&reference.from)
                .display(),
            reference.line,
            reference.raw
        ));
    }
    Ok(warnings)
}

fn name(wm: crate::manifest::Wm) -> String {
    format!("{wm:?}").to_lowercase()
}

/// Hooks run on a bundle's **first activation only** (invariant 13). Real hooks append to
/// files and appending twice is not undoable, so the ledger is what decides, not the
/// presence of the field.
fn hooks(bundle: &Bundle, ledger: &Ledger, options: Options) -> (Vec<Hook>, Option<String>) {
    let Some(declared) = &bundle.manifest.hooks else {
        return (Vec::new(), None);
    };
    if options.no_hooks {
        return (Vec::new(), Some("hooks skipped (--no-hooks)".into()));
    }
    if ledger.hooks_ran.contains(&bundle.manifest.name) && !options.run_hooks {
        return (
            Vec::new(),
            Some("this bundle's hooks have already run — `--run-hooks` runs them again".into()),
        );
    }

    let mut hooks = Vec::new();
    for (when, path) in [
        ("pre_install", &declared.pre_install),
        ("post_install", &declared.post_install),
    ] {
        let Some(path) = path else { continue };
        hooks.push(Hook {
            when,
            path: path.clone(),
            // Unreadable is not fatal here: the plan says so and `system::hook` reports
            // the failure when it tries to run it.
            script: std::fs::read_to_string(bundle.root.join(path))
                .unwrap_or_else(|e| format!("<cannot be read: {e}>")),
        });
    }
    (hooks, None)
}

/// profiles.md §3, `use B`. A first activation is the same sequence with an empty ledger.
pub fn switch(bundle: &Bundle, plan: Plan) -> Result<Summary> {
    let mut ledger = Ledger::load()?;
    let mut summary = Summary::default();
    let stamp = backup::stamp();

    // 1. pre_install, before the packages: a hook that adds a repo has to run before the
    //    thing it exists to prepare for. The two hooks are not one step (design.md §4.2).
    run_hooks(&plan, "pre_install", bundle, &mut summary.notes);

    // 2. Packages, before the links — a config for a program that is not installed yet is
    //    harmless.
    summary.packages_failed = pkg::install(&plan.packages)?;

    // 3. The link diff.
    let new = bundle.links()?;
    let (remove, place) = links::diff(&ledger.links, &new);
    let mut placed = Vec::new();
    for entry in &remove {
        links::remove(entry, &mut summary.notes)?;
    }
    summary.unlinked = remove.len();
    for (link, previous) in &place {
        placed.push(links::place(link, *previous, &stamp, &mut summary.notes)?);
    }
    summary.linked = placed.len();

    // 4. Fonts, then services, then the ledger, then the WM.
    let touched: Vec<PathBuf> = new.iter().map(|l| l.target.clone()).collect();
    system::refresh_fonts(&touched, &mut summary.notes)?;

    let wanted = &bundle.manifest.services;
    let stale: Vec<String> = ledger
        .services
        .iter()
        .filter(|s| !wanted.contains(s))
        .cloned()
        .collect();
    system::services(wanted, &stale, &mut summary.notes);

    // 5. post_install, after the links and the services — and only then does the ledger
    //    record that this bundle's hooks have run.
    run_hooks(&plan, "post_install", bundle, &mut summary.notes);
    if !plan.hooks.is_empty() && !ledger.hooks_ran.contains(&plan.name) {
        ledger.hooks_ran.push(plan.name.clone());
    }

    if ledger.active.as_ref() != Some(&plan.name) {
        ledger.previous = ledger.active.take();
    }
    ledger.active = Some(plan.name.clone());
    ledger.activated_at = Some(ledger::now());
    ledger.services = wanted.clone();
    ledger.links = placed;
    ledger.save()?;

    if backup_taken(&ledger, &stamp) {
        summary.backup_dir = Some(paths::backups().join(&stamp));
    }
    system::reload(bundle.manifest.wm, &mut summary.notes);
    Ok(summary)
}

/// Remove every link and leave nothing active — where `use -` lands when there is no
/// previous bundle, because the state before the first activation was exactly this.
pub fn deactivate() -> Result<Summary> {
    let mut ledger = Ledger::load()?;
    let mut summary = Summary::default();

    for entry in &ledger.links {
        links::remove(entry, &mut summary.notes)?;
    }
    summary.unlinked = ledger.links.len();
    system::services(&[], &ledger.services.clone(), &mut summary.notes);

    ledger.previous = ledger.active.take();
    ledger.activated_at = Some(ledger::now());
    ledger.services.clear();
    ledger.links.clear();
    ledger.save()?;
    Ok(summary)
}

/// Re-place the links of the active bundle whose targets no longer hold them. The
/// write-back half — putting the application's version into the bundle — is M2, because
/// a file entering the bundle has to pass the secret scan first.
pub fn sync(discard: bool) -> Result<Summary> {
    let mut ledger = Ledger::load()?;
    let mut summary = Summary::default();
    let Some(name) = ledger.active.clone() else {
        bail!("nothing is active");
    };
    let bundle = Bundle::open(paths::store().join(&name))?;
    let stamp = backup::stamp();

    let mut repaired = ledger.links.clone();
    for entry in &mut repaired {
        let state = links::state(entry, &bundle.root);
        if state == links::State::Linked {
            continue;
        }
        let Some(link) = bundle
            .links()?
            .into_iter()
            .find(|l| paths::contract(&l.target) == entry.target)
        else {
            summary
                .notes
                .push(format!("{} is no longer in the bundle", entry.target));
            continue;
        };
        if state == links::State::Detached {
            let target = paths::expand(&entry.target);
            // An application replaced our link with a real file, and its version is
            // normally the one that is wanted — that is the whole reason sync exists.
            let refused = if discard {
                Vec::new()
            } else {
                write::write_back(&target, &link.source)?
            };
            if refused.is_empty() {
                summary.notes.push(format!(
                    "{} was detached — {}",
                    entry.target,
                    if discard {
                        "backed up, and the bundle's version relinked"
                    } else {
                        "written back into the bundle, then relinked"
                    }
                ));
            } else {
                summary.notes.push(format!(
                    "{} was NOT written back — it would put a secret in the bundle:",
                    entry.target
                ));
                summary
                    .notes
                    .extend(refused.into_iter().map(|r| format!("    {r}")));
                continue;
            }
        }
        *entry = links::place(&link, Some(entry), &stamp, &mut summary.notes)?;
        summary.linked += 1;
    }

    ledger.links = repaired;
    ledger.save()?;
    Ok(summary)
}

/// `rm` refuses while the bundle is active: deactivating on the user's behalf is
/// destruction they did not ask for (TODO.md Phase 0).
pub fn remove_bundle(name: &str) -> Result<()> {
    let ledger = Ledger::load()?;
    if ledger.active.as_deref() == Some(name) {
        bail!("{name} is active — `dotpack use -` first");
    }
    let path = paths::store().join(name);
    // A local-path bundle is a link in the store; removing it must not touch the repo.
    if path.symlink_metadata()?.file_type().is_symlink() {
        std::fs::remove_file(&path)?;
    } else {
        std::fs::remove_dir_all(&path)?;
    }
    Ok(())
}

fn run_hooks(plan: &Plan, when: &str, bundle: &Bundle, notes: &mut Vec<String>) {
    for hook in plan.hooks.iter().filter(|h| h.when == when) {
        notes.push(format!("ran {when} hook {}", hook.path));
        system::hook(&bundle.root, &hook.path, bundle.manifest.mode, notes);
    }
}

fn manual_steps(bundle: &Bundle) -> Vec<String> {
    bundle
        .manifest
        .components
        .iter()
        .filter_map(|(role, component)| match component {
            Component::Full(detail) => detail
                .url
                .as_ref()
                .map(|url| format!("{role}: {} — {url}", detail.name.as_deref().unwrap_or(role))),
            Component::Pkg(_) => None,
        })
        .collect()
}

fn backup_taken(ledger: &Ledger, stamp: &str) -> bool {
    ledger
        .links
        .iter()
        .filter_map(|l| l.adopted_backup.as_ref())
        .any(|b| b.starts_with(stamp))
}

/// A local path is not copied into the store, it is **linked** into it: the file you edit
/// stays the file in your repo. Everything under `bundles/` is then a directory as far as
/// `ls` / `use` / `rm` are concerned (TODO.md Phase 0).
pub fn link_into_store(path: &std::path::Path, as_name: Option<&str>) -> Result<String> {
    let bundle = Bundle::open(std::fs::canonicalize(path)?)?;
    bundle.manifest.validate()?;
    let name = match as_name {
        Some(given) if !crate::manifest::valid_name(given) => {
            bail!("`{given}` is not a valid bundle name ([a-z0-9._-]+)")
        }
        Some(given) => given.to_string(),
        None => bundle.manifest.name.clone(),
    };
    let entry = paths::store().join(&name);

    match entry.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() && std::fs::read_link(&entry)? == bundle.root => {
        }
        Ok(_) => bail!("the store already has a bundle called `{name}` — `--as <other-name>`"),
        Err(_) => {
            std::fs::create_dir_all(paths::store())?;
            std::os::unix::fs::symlink(&bundle.root, &entry)?;
        }
    }
    Ok(name)
}
