<div align="center">

# dotpack

**Bundle your dotfiles together with the packages they need.**<br>
Install them with one command, switch between rices instantly.

[![crates.io](https://img.shields.io/crates/v/dotpack?color=green&label=crates.io)](https://crates.io/crates/dotpack)
[![license](https://img.shields.io/badge/license-GPL--3.0-blue)](LICENSE)
[![rust](https://img.shields.io/badge/rust-1.88%2B-orange)](https://www.rust-lang.org)
[![built with Claude Code](https://img.shields.io/badge/built%20with-Claude%20Code-8A63D2)](https://claude.com/claude-code)

Arch Linux · hyprland / sway / i3 · Rust + ratatui

</div>

```bash
dotpack collect                # scan this machine, write a bundle
dotpack use ~/dotfiles         # activate it: links, packages, services
dotpack use github:u/repo      # or somebody else's, straight from git
dotpack use -                  # didn't like it, go back
dotpack post                   # the r/unixporn list, generated from the manifest
```

> **Status: all of it works.** Seven commands, `external` mode for repos chezmoi or stow
> already manages, and a TUI over the same functions. M0–M7 in [TODO.md](TODO.md), tested
> on a second Arch machine with no AUR helper. The one thing never tested anywhere is the
> compositor reload — `hyprctl reload` needs a running session, and the lab runs over ssh.

## Install

```bash
git clone https://github.com/murat-akpinar/dotpack
cd dotpack && makepkg -si
```

A `PKGBUILD` and not `cargo install`, for the reason the tool exists: on Arch, software
arrives as a package. `pacman -R dotpack-git` then removes it, which a binary dropped in
`~/.cargo/bin` does not. Being a `-git` package it builds the latest commit on GitHub
rather than your checkout, and it runs the test suite on the way — the tests stub `pacman`
and use a temporary `HOME`, so building never touches the machine building it.

It is on [crates.io](https://crates.io/crates/dotpack) as well, so `cargo install dotpack`
works — but that is the second-best route here and the paragraph above is why. Use it if
you are not on Arch and want to read the code, which is the only case where a binary
outside the package manager is the right answer for this particular tool.

## The problem

Sharing dotfiles today means: clone the repo, read the README, install 40 packages by
hand, then figure out what's still missing from the error messages. What breaks isn't
the files — it's the *environment* the files need: packages, fonts, themes, script
dependencies.

Existing tools each solve half of it. chezmoi, yadm and stow manage files but know
nothing about packages. pacdef and decman manage packages but not files. ML4W and
JaKooLit ship both, but welded to one specific rice. Nobody reads your config and
works out the dependencies for you.

## Commands

| | |
|---|---|
| `collect` | Scan this machine's configs and packages, write a bundle |
| `add` | Download a bundle into the local store, without installing it |
| `use` | Make a bundle active — the rice switch. `-` returns to the previous one |
| `ls` | Bundles in the local store, and which one is active |
| `sync` | Repair links an application replaced with a real file |
| `post` | Render a bundle's `components` as a shareable list |
| `rm` | Remove a bundle from the store |

`dotpack` with no arguments opens the TUI, and every screen calls the same function the
CLI calls — there is one implementation of "switch a bundle", not two:

```
┌ dotpack ─────────────────────────────────────────────────────────────────────┐
│● imperative-hyprland    hyprland   76 pkgs  active · 2 detached ⚠1           │
│○ caelestia              hyprland   51 pkgs  github:caelestia-dots/shell      │
│○ minimal-sway           sway       12 pkgs                                   │
│                                                                              │
│imperative-hyprland                                                           │
│imperative-dots, customized — quickshell UI, matugen colors, fish             │
│v0.1.0 · GPL-3.0 · murat-akpinar · ~/.local/share/dotpack/bundles/…           │
│config: cava, fastfetch, fish, hypr, kitty   +4                               │
│                                                                              │
│↵ switch  a add  c collect  s sync  d delete  - back  ? help  q quit          │
└──────────────────────────────────────────────────────────────────────────────┘
```

`●` is the active bundle, `2 detached` means an application replaced a symlink with a
real file — the one signal that points you at `sync` — and `⚠1` is the secret scan
finding something inside the bundle you edit every day. Installing packages leaves the
TUI on purpose: pacman prints its own output and the sudo prompt is a real one.

## What it does

`collect` scans your machine, follows your config to find which commands it actually
launches and which files it includes, resolves those to packages, and writes a
**bundle**: a plain directory you can push to git.

```
awesome-rice/
├── dotfiles.toml        # package lists + a few settings
├── README.md            # generated from the manifest, then yours to edit
├── config/  → ~/.config/
├── home/    → ~/
├── local/   → ~/.local/
├── assets/  → destinations declared in dotfiles.toml
└── hooks/
```

There is no file list in the manifest — where a file lands is implied by the
directory it sits in. That keeps `dotfiles.toml` small enough to write by hand:

```toml
name = "my-hyprland"
wm   = "hyprland"

[packages]
pacman = ["hyprland", "waybar", "kitty"]
yay    = ["matugen-bin"]
```

`use` activates a bundle by pointing symlinks at it, so switching rices is instant.
A link ledger records exactly what was placed, so switching away is clean and your
own pre-existing configs are backed up rather than clobbered.

`post` renders the manifest's `[components]` as the list every r/unixporn comment already
has — WM, bar, terminal, fonts, themes, who you took the theme from — and copies it to the
clipboard. `collect` writes the same list into the bundle's `README.md`. That is the whole
adoption argument: people write this list by hand anyway, so the format should produce it.

Two things it takes seriously because they are the usual ways a shared rice arrives
broken: **every referenced file has to ship** — a `kitty.conf` whose
`include catppuccin.conf` is missing installs a kitty that errors on every start — and
**fonts have to actually arrive**, resolved to a package where one exists and shipped in
the bundle where none does, with `fc-cache` run afterwards.

## The format

`spec/` is the bundle format, and it is written to be read without this tool — one
directory, three documents, no dependency on anything in `docs/`.

| | |
|---|---|
| [spec/README.md](spec/README.md) | Layout, where each file lands, and what an installer must do |
| [spec/manifest.md](spec/manifest.md) | `dotfiles.toml` — every field and the validation rules |
| [spec/components.md](spec/components.md) | The role vocabulary: `bar`, `terminal`, `icons`, `font_system`… |

## Docs

The tool's own notes. Nothing here is normative.

| | |
|---|---|
| [docs/research.md](docs/research.md) | Prior art, gap analysis, verified techniques |
| [docs/real-world.md](docs/real-world.md) | Teardown of a real shared rice, and what it changed |
| [docs/design.md](docs/design.md) | Flows, dependency discovery, Rust layout |
| [docs/profiles.md](docs/profiles.md) | Local store, rice switching, `github:` syntax |
| [docs/tui.md](docs/tui.md) | Screens, keymap, ratatui decisions |
| [example/](example/) | A real rice as a bundle — the 1898-line installer it replaces |

`example/` is published on its own at
[murat-akpinar/dotpack-example](https://github.com/murat-akpinar/dotpack-example), which
is what `dotpack use github:murat-akpinar/dotpack-example` clones. Same directory, pushed
with `git subtree push --prefix=example`.

The build order lives in [TODO.md](TODO.md): eight milestones, each one written to leave a
working tool behind rather than a half-finished layer, and each carrying a note afterwards
on what it cost and what it got wrong.

## How it was built

dotpack was written with [Claude Code](https://claude.com/claude-code), pair-programming
rather than prompting: the design in `docs/`, the format in `spec/` and every milestone
note in `TODO.md` came out of that back-and-forth, and each one records what it cost and
what it got wrong. The parts worth reading for that are the teardown in
[docs/real-world.md](docs/real-world.md) — a real shared rice pulled apart, which is where
several of the design rules come from — and the findings list in
[example/README.md](example/README.md), written at the moment each one was found rather
than tidied up afterwards.

The commits are the author's own. Nothing here was accepted without being run: the
milestone notes name the bugs that only showed up on a second machine, and the ones that
were shipped and then caught by a check written afterwards.
