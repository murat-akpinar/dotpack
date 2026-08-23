# dotpack

Bundle your dotfiles **together with the packages they need**, install them with one
command, and switch between rices instantly.

```bash
dotpack use github:caelestia-dots/shell   # fetch + activate
dotpack use -                             # didn't like it, go back
dotpack collect                           # package up your own setup
```

Arch Linux · hyprland / sway / i3 · Rust + ratatui

> **Status: design phase. No code yet.**

## The problem

Sharing dotfiles today means: clone the repo, read the README, install 40 packages by
hand, then figure out what's still missing from the error messages. What breaks isn't
the files — it's the *environment* the files need: packages, fonts, themes, script
dependencies.

Existing tools each solve half of it. chezmoi, yadm and stow manage files but know
nothing about packages. pacdef and decman manage packages but not files. ML4W and
JaKooLit ship both, but welded to one specific rice. Nobody reads your config and
works out the dependencies for you.

## What it does

`collect` scans your machine, follows your config to find which commands it actually
launches and which files it includes, resolves those to packages, and writes a
**bundle**: a plain directory you can push to git.

```
awesome-rice/
├── dotfiles.toml        # package lists + a few settings
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

Two things it takes seriously because they are the usual ways a shared rice arrives
broken: **every referenced file has to ship** — a `kitty.conf` whose
`include catppuccin.conf` is missing installs a kitty that errors on every start — and
**fonts have to actually arrive**, resolved to a package where one exists and shipped in
the bundle where none does, with `fc-cache` run afterwards.

## Docs

| | |
|---|---|
| [docs/research.md](docs/research.md) | Prior art, gap analysis, verified techniques |
| [docs/standard.md](docs/standard.md) | The `components` standard and `dotpack post` |
| [docs/real-world.md](docs/real-world.md) | Teardown of a real shared rice, and what it changed |
| [docs/design.md](docs/design.md) | Directory format, flows, dependency discovery, Rust layout |
| [docs/manifest.md](docs/manifest.md) | `dotfiles.toml` full schema reference |
| [docs/profiles.md](docs/profiles.md) | Local store, rice switching, `github:` syntax |
| [docs/tui.md](docs/tui.md) | Screens, keymap, ratatui decisions |
| [example/](example/) | A real rice as a bundle — the 1898-line installer it replaces |

The build order lives in [TODO.md](TODO.md): seven milestones, each leaving a usable tool
behind, and one Phase 0 list of the questions still open.
