mod apply;
mod bundle;
mod manifest;
mod paths;
mod pkg;

use std::io::Write;
use std::path::Path;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};

use apply::ledger::Ledger;
use bundle::Bundle;

#[derive(Parser)]
#[command(
    name = "dotpack",
    version,
    about = "Dotfiles, bundled with the packages they need"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Scan this machine's configs and packages, write a bundle
    Collect,
    /// Download a bundle into the local store, without installing it
    Add { source: String },
    /// Make a bundle active — the rice switch. `-` returns to the previous one
    Use {
        /// A name in the store, a local path, or `-`
        target: String,
        /// Skip the confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Bundles in the local store, and which one is active
    Ls,
    /// Repair links an application replaced with a real file
    Sync,
    /// Render a bundle's `components` as a shareable list
    Post { name: Option<String> },
    /// Remove a bundle from the store
    Rm { name: String },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Some(Command::Use { target, yes }) => use_bundle(&target, yes),
        Some(Command::Ls) => list(),
        Some(Command::Sync) => {
            report(apply::sync()?);
            Ok(())
        }
        Some(Command::Rm { name }) => apply::remove_bundle(&name),
        Some(Command::Collect) => bail!("collect: M2"),
        Some(Command::Post { .. }) => bail!("post: M3"),
        Some(Command::Add { .. }) => bail!("add: M4"),
        None => bail!("the TUI is M6 — use a subcommand for now (`dotpack --help`)"),
    }
}

// --- use start ---

fn use_bundle(target: &str, yes: bool) -> Result<()> {
    if target == "-" {
        return use_previous(yes);
    }
    if let Some(prefix) = ["github:", "gitlab:", "https://", "http://", "git@"]
        .iter()
        .find(|p| target.starts_with(**p))
    {
        bail!("remote sources ({prefix}…) are M4 — add the bundle by path for now");
    }

    let name = if target.contains('/') || Path::new(target).exists() {
        apply::link_into_store(Path::new(target))?
    } else {
        target.to_string()
    };
    activate(&Bundle::open(paths::store().join(&name))?, yes)
}

fn use_previous(yes: bool) -> Result<()> {
    let ledger = Ledger::load()?;
    match ledger.previous {
        Some(name) => activate(&Bundle::open(paths::store().join(&name))?, yes),
        // The state before the first activation was "nothing placed", so that is where
        // going back with no previous bundle lands.
        None if ledger.active.is_some() => {
            println!("no previous bundle — this removes every link and leaves none active");
            if !yes && !confirm()? {
                return Ok(());
            }
            report(apply::deactivate()?);
            Ok(())
        }
        None => bail!("nothing is active"),
    }
}

fn activate(bundle: &Bundle, yes: bool) -> Result<()> {
    let plan = apply::plan(bundle)?;
    show(&plan);
    // `enter` is never destructive: the plan is shown, and applying it is a second step.
    if !yes && !confirm()? {
        println!("nothing done");
        return Ok(());
    }
    report(apply::switch(bundle, plan)?);
    Ok(())
}

fn show(plan: &apply::Plan) {
    println!("plan: {}", plan.name);
    let p = &plan.packages;
    if !p.is_empty() {
        println!(
            "  install   {} from repos, {} from the AUR",
            p.repo.len(),
            p.aur.len()
        );
        if !p.unknown.is_empty() {
            println!(
                "            no repo has {} — trying the AUR",
                p.unknown.join(", ")
            );
        }
        if p.helper.is_none() && !p.aur.is_empty() {
            println!("            no AUR helper (paru/yay/pikaur/trizen) — those are skipped");
        }
    }
    for (label, list) in [
        ("link", &plan.place),
        ("unlink", &plan.remove),
        ("detached", &plan.detached),
        ("manual", &plan.manual),
        ("warning", &plan.warnings),
    ] {
        for (i, item) in list.iter().enumerate() {
            println!("  {:<9} {item}", if i == 0 { label } else { "" });
        }
    }
    if plan.hooks_declared {
        println!("  hooks     declared, not run (M4)");
    }
}

fn report(summary: apply::Summary) {
    println!("{} linked, {} removed", summary.linked, summary.unlinked);
    if !summary.packages_failed.is_empty() {
        println!("not installed: {}", summary.packages_failed.join(", "));
    }
    if let Some(dir) = summary.backup_dir {
        println!("backed up to {}", dir.display());
    }
    for note in summary.notes {
        println!("  {note}");
    }
}

fn confirm() -> Result<bool> {
    print!("apply? [y/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes"))
}

// --- use end ---

fn list() -> Result<()> {
    let ledger = Ledger::load()?;
    let bundles = bundle::store_list()?;
    if bundles.is_empty() {
        println!("no bundles — `dotpack use <path>` adds one");
        return Ok(());
    }
    for path in bundles {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let active = ledger.active.as_deref() == Some(&name);
        match Bundle::open(&path) {
            Ok(b) => {
                let m = &b.manifest;
                let count = m.packages.pacman.len() + m.packages.yay.len() + m.packages.paru.len();
                let mut state = if active {
                    "active".to_string()
                } else {
                    String::new()
                };
                if active {
                    let detached = ledger
                        .links
                        .iter()
                        .filter(|e| {
                            apply::links::state(e, &b.root) == apply::links::State::Detached
                        })
                        .count();
                    if detached > 0 {
                        state.push_str(&format!(" · {detached} detached"));
                    }
                }
                println!(
                    "{} {name:<24} {:<9} {count:>3} packages   {state}",
                    if active { "●" } else { "○" },
                    format!("{:?}", m.wm).to_lowercase(),
                );
            }
            Err(e) => println!("✗ {name:<24} {e}"),
        }
    }
    Ok(())
}
