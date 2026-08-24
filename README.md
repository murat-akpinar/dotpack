# dotpack

Bundle your dotfiles **together with the packages they need**, install them with one
command, and switch between rices instantly.

```bash
dotpack collect                # scan this machine, write a bundle
dotpack use ~/dotfiles         # activate it: links, packages, services
dotpack use github:u/repo      # or somebody else's, straight from git
dotpack use -                  # didn't like it, go back
dotpack post                   # the r/unixporn list, generated from the manifest
```

Arch Linux · hyprland / sway / i3 · Rust + ratatui

> **Status: all of it works.** Every verb above, plus `add`, `ls`, `sync` and `rm`;
> `external` mode for repos chezmoi or stow already manages; and a TUI over the same
> functions, which `dotpack` with no arguments opens. M0–M7 in [TODO.md](TODO.md), tested
> on a second Arch machine with no AUR helper. What has never been tested anywhere is the
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

The build order lives in [TODO.md](TODO.md): eight milestones, each leaving a usable tool
behind, and one Phase 0 list of the questions still open.
