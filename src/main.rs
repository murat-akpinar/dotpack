mod apply;
mod bundle;
mod manifest;
mod paths;
mod pkg;
mod post;
mod scan;

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
    Collect {
        /// Directories under ~/.config. With none given: the WM's own, plus the ones its
        /// config starts or points at
        dirs: Vec<String>,
        /// Where to write. Default: ~/dotfiles
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        /// The bundle's name. Default: <user>-<wm>
        #[arg(long)]
        name: Option<String>,
        /// Override WM detection
        #[arg(long)]
        wm: Option<String>,
        /// Keep a path out of the bundle. Repeatable, collect-time only
        #[arg(long)]
        ignore: Vec<String>,
        /// Do not `git init` the result
        #[arg(long)]
        no_git: bool,
        /// Skip the confirmation
        #[arg(short, long)]
        yes: bool,
    },
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
    Sync {
        /// Keep the bundle's version instead of the application's
        #[arg(long)]
        discard: bool,
    },
    /// Render a bundle's `components` as a shareable list
    Post {
        /// A name in the store or a local path. Default: the active bundle
        name: Option<String>,
        #[arg(long, value_enum, default_value = "reddit")]
        format: post::Format,
    },
    /// Remove a bundle from the store
    Rm { name: String },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Some(Command::Use { target, yes }) => use_bundle(&target, yes),
        Some(Command::Ls) => list(),
        Some(Command::Sync { discard }) => {
            report(apply::sync(discard)?);
            Ok(())
        }
        Some(Command::Rm { name }) => apply::remove_bundle(&name),
        Some(Command::Collect {
            dirs,
            out,
            name,
            wm,
            ignore,
            no_git,
            yes,
        }) => collect(&dirs, out, name, wm, &ignore, !no_git, yes),
        Some(Command::Post { name, format }) => post(name, format),
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
            if !yes && !confirm("apply")? {
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
    if !yes && !confirm("apply")? {
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

fn confirm(verb: &str) -> Result<bool> {
    print!("{verb}? [y/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes"))
}

// --- use end ---

// --- collect start ---

fn collect(
    dirs: &[String],
    out: Option<std::path::PathBuf>,
    name: Option<String>,
    wm: Option<String>,
    ignore: &[String],
    git: bool,
    yes: bool,
) -> Result<()> {
    let wm = match wm {
        Some(given) => scan::wm::parse(&given)
            .ok_or_else(|| anyhow::anyhow!("unknown wm `{given}` — hyprland, sway or i3"))?,
        None => scan::wm::detect()
            .ok_or_else(|| anyhow::anyhow!("could not tell which WM this is — pass --wm"))?,
    };
    let collected = scan::collect(dirs, ignore, name, wm)?;
    let out = out.unwrap_or_else(|| paths::home().join("dotfiles"));

    println!("collect: {} ({wm:?})", collected.manifest.name);
    for (directory, count) in counts(&collected) {
        println!("  {directory:<22} {count:>3} files");
    }
    let p = &collected.manifest.packages;
    println!(
        "  packages               {:>3} from repos, {} from the AUR",
        p.pacman.len(),
        p.yay.len() + p.paru.len()
    );
    // Every suggestion carries the line that produced it. Seeing that `texinfo` came from
    // one `info "…"` call is the difference between weeding the list and trusting it.
    for suggestion in &collected.suggestions {
        println!(
            "    {:<24} {}{}",
            suggestion.package,
            suggestion.reason,
            suggestion
                .note
                .as_ref()
                .map(|n| format!(" — {n}"))
                .unwrap_or_default()
        );
    }
    for finding in &collected.secrets {
        println!(
            "  secret     {}:{} {}",
            paths::contract(&finding.file),
            finding.line,
            finding.what
        );
    }
    for reference in &collected.dangling {
        println!(
            "  dangling   {}:{} → {} ({:?})",
            paths::contract(&reference.from),
            reference.line,
            reference.raw,
            reference.verdict
        );
    }
    for warning in &collected.warnings {
        println!("  warning    {warning}");
    }
    println!("→ {}", out.display());

    if !yes && !confirm("write")? {
        println!("nothing written");
        return Ok(());
    }
    for note in apply::write::write_bundle(&collected, &out, git)? {
        println!("  {note}");
    }
    println!("wrote {} — {} files", out.display(), collected.files.len());
    Ok(())
}

/// Files per top-level directory in the bundle, in the order they will appear in it.
fn counts(collected: &scan::Collected) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for (_, relative) in &collected.files {
        let directory = relative
            .parent()
            .map(|p| {
                p.components()
                    .take(2)
                    .collect::<std::path::PathBuf>()
                    .display()
                    .to_string()
            })
            .unwrap_or_default();
        match counts.iter_mut().find(|(name, _)| *name == directory) {
            Some((_, count)) => *count += 1,
            None => counts.push((directory, 1)),
        }
    }
    counts
}

// --- collect end ---

// --- post start ---

fn post(name: Option<String>, format: post::Format) -> Result<()> {
    let bundle = match name {
        // A path renders without the bundle being in the store: the manifest is the only
        // input, so there is nothing to install first.
        Some(target) if target.contains('/') || Path::new(&target).is_dir() => {
            Bundle::open(std::fs::canonicalize(target)?)?
        }
        Some(target) => Bundle::open(paths::store().join(target))?,
        None => {
            let active = Ledger::load()?
                .active
                .ok_or_else(|| anyhow::anyhow!("nothing is active — `dotpack post <name>`"))?;
            Bundle::open(paths::store().join(active))?
        }
    };
    for warning in bundle.manifest.validate()? {
        eprintln!("warning: {warning}");
    }

    let text = post::render(&bundle.manifest, format);
    println!("{text}");
    if post::copy(&text) {
        println!("\n[copied to clipboard]");
    }
    Ok(())
}

// --- post end ---

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
                    // In symlink mode the active bundle's files are the ones being
                    // edited, so a token added the day after `collect` is seen by nothing
                    // unless this looks too (design.md §6).
                    let secrets = scan::secrets::scan(&b.files()).len();
                    if secrets > 0 {
                        state.push_str(&format!(" · {secrets} secret"));
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
