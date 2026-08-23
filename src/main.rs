// ponytail: crate-wide allow while the command bodies are empty — every path in paths.rs
// has a caller in M1. Delete this line when `use` is wired up.
#![allow(dead_code)]

mod bundle;
mod manifest;
mod paths;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};

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
    Use { name: String },
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
    // M0 is the manifest and the layout rules; the verbs land in the milestone named here.
    let milestone = match Cli::parse().command {
        Some(Command::Use { .. } | Command::Ls | Command::Sync | Command::Rm { .. }) => "M1",
        Some(Command::Collect) => "M2",
        Some(Command::Post { .. }) => "M3",
        Some(Command::Add { .. }) => "M4",
        None => "M6, the TUI — use a subcommand for now (`dotpack --help`)",
    };
    bail!("not implemented yet — {milestone}");
}
