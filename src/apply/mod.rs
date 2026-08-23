//! **The only module that writes to disk.** Everything else reads and proposes.
//!
//! This file holds the sequences and nothing else, so that design.md §4.2's thirteen
//! steps and profiles.md §3's ten-step switch stay readable as the code that runs them.

pub mod backup;
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
    pub hooks_declared: bool,
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
pub fn plan(bundle: &Bundle) -> Result<Plan> {
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

    Ok(Plan {
        name: bundle.manifest.name.clone(),
        packages: pkg::plan(&bundle.manifest)?,
        place: place
            .iter()
            .map(|(l, _)| paths::contract(&l.target))
            .collect(),
        remove: remove.iter().map(|e| e.target.clone()).collect(),
        detached,
        warnings: bundle.manifest.validate()?,
        manual: manual_steps(bundle),
        hooks_declared: bundle.manifest.hooks.is_some(),
    })
}

/// profiles.md §3, `use B`. A first activation is the same sequence with an empty ledger.
pub fn switch(bundle: &Bundle, plan: Plan) -> Result<Summary> {
    let mut ledger = Ledger::load()?;
    let mut summary = Summary::default();
    let stamp = backup::stamp();

    // 1. Hooks are M4. A bundle that declares one is told, not silently skipped.
    if plan.hooks_declared {
        summary
            .notes
            .push("this bundle declares hooks; running them is M4, so they were skipped".into());
    }

    // 2. Packages, before the links — a config for a program that is not installed yet is
    //    harmless, and pre_install will need to run before this in M4.
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
pub fn sync() -> Result<Summary> {
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
            summary.notes.push(format!(
                "{} was detached — the file was backed up",
                entry.target
            ));
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
pub fn link_into_store(path: &std::path::Path) -> Result<String> {
    let bundle = Bundle::open(std::fs::canonicalize(path)?)?;
    bundle.manifest.validate()?;
    let name = bundle.manifest.name.clone();
    let entry = paths::store().join(&name);

    match entry.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() && std::fs::read_link(&entry)? == bundle.root => {
        }
        Ok(_) => bail!("the store already has a bundle called `{name}` (`--as` is M4)"),
        Err(_) => {
            std::fs::create_dir_all(paths::store())?;
            std::os::unix::fs::symlink(&bundle.root, &entry)?;
        }
    }
    Ok(name)
}
