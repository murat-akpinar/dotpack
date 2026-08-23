# CLAUDE.md

Project instructions for Claude Code working in this repo.

## What this is

`dotpack` bundles dotfiles together with the packages they need, installs
them in one command, and switches between rices by re-pointing symlinks.

Rust + ratatui · Arch only · hyprland / sway / i3.

**Currently design-phase: `docs/` is written, `src/` does not exist yet.** Read
`docs/design.md` before proposing any structure, and `docs/real-world.md` for what
real shared rices actually look like (several design rules come from that teardown).

**Everything in this repo is written in English** — docs, filenames, code, comments,
commit messages. The user writes in Turkish; the files do not.

## Terminology

- **bundle** — a directory in the shared format (`dotfiles.toml` + `config/` + …). One git repo = one bundle.
- **store** — `~/.local/share/dotpack/bundles/`, where bundles live locally.
- **active** — the one bundle whose symlinks are currently in place. Exactly one, never partial.
- **external mode** — `mode = "external"`: the bundle ships no files, only `packages` + `components`. Files stay under chezmoi/stow. This is a v1 feature and the format's main distribution path, not a fallback.
- **components** — optional role→component map in `dotfiles.toml` (`bar`, `terminal`, `icons`…). Descriptive only; install logic reads `packages`. This is the standard the project is trying to set — see `docs/standard.md`.
- **link ledger** — `~/.local/state/dotpack/state.toml`, the record of every link placed. Switching cleanly depends entirely on this file being accurate.

## Invariants — do not violate

1. **`scan/` never writes to disk.** It reads and returns suggestions. `apply.rs` is
   the only module that mutates the filesystem. Keep it that way; it is what makes
   scanning testable and safe.
2. **Nothing is destroyed without a backup.** Any real file at a target path is moved
   to `~/.local/state/dotpack/backups/<timestamp>/` and recorded in the ledger
   so it can be restored.
3. **Packages are never uninstalled.** Switching bundles only installs what's missing.
4. **Secret scanning is not simplifiable.** The deny-list and content patterns in
   `docs/design.md` §6 stay. Leaking a token through a shared dotfile is this
   project's biggest risk.
5. **Hook scripts are shown to the user before they run.** They come from other
   people's repos.
6. **Package installation happens outside the TUI.** Restore the terminal, let pacman
   print its own output, re-enter. Do not reimplement pacman's progress UI.
7. **`enter` is never destructive.** It leads to a plan screen; the plan screen's
   `enter` applies.
8. **Never run `pacman -Syu`.** Only `pacman -S --needed`. A dotfile installer must
   not upgrade someone's system behind their back. Real installers do this; we don't.
9. **Copying preserves mode bits.** Use `fs::copy`, never read+write. Rices ship
   dozens of executable scripts; a lost exec bit breaks them silently.
10. **A repo without `dotfiles.toml` is rejected with a clear message.** Do not add a
   fallback that runs a foreign `install.sh`. See `docs/real-world.md` § adoption.

## Link depth rule

Walk down from `config/`. If a directory contains files directly, link it and stop.
If it only contains directories, descend one more level.

- `config/hypr/hyprland.conf` → link `~/.config/hypr`
- `config/hypr/themes/x/*` (nothing above it) → link `~/.config/hypr/themes/x`

The second case is real: rices install *alongside* a user's own config, not over it.

## Dependencies

Current set — `ratatui`, `crossterm`, `serde`, `toml`, `clap`, `walkdir`, `anyhow`.

The manifest is TOML because humans hand-write it and JSON has no comments. Tool-written
state (`state.toml`) uses the same format so there is one parser, not two.

Deliberately absent. Do not add these without asking:

| Not used | Instead |
|---|---|
| `git2` | shell out to `git` |
| `alpm` bindings | parse `pacman` / `expac` output |
| `tokio` | one worker thread + `std::sync::mpsc` |
| `dirs` | `std::env::var("HOME")` |
| `regex` | `split('=')` + `split_whitespace()` |
| `tui-tree-widget` | flat lists, not trees |

Before adding any crate, answer: how many lines would I write without it? Under ~50
means write the lines.

## Style

- Follow the ladder: does it need to exist → stdlib → already-installed dep → one line → minimum code.
- Mark deliberate shortcuts with a `ponytail:` comment naming the ceiling and the upgrade path. Several are already recorded in the docs — carry them into the code.
- Non-trivial logic leaves one runnable check behind (`#[test]` next to the code). No test frameworks, no fixtures.
- TUI colors use terminal palette constants (`Color::Green`), never hardcoded RGB. A ricing tool must not override the user's theme.
- `std::panic::set_hook` must restore the terminal before printing. Non-negotiable.

## Git

- **Commits carry the user's identity only.** Never add `Co-Authored-By: Claude`, a
  `Claude-Session:` trailer, or a "Generated with Claude Code" line — not in commits, not
  in PR bodies, not in changelog entries.
- **Conventional commits.** `cliff.toml` sets `filter_unconventional = true`, so a commit
  that does not start with `feat:` / `fix:` / `docs:` / `refactor:` / `chore:` … is
  dropped from the changelog entirely. Scope in parens: `docs(manifest): …`.
- **Regenerate the changelog with the commit that changes behavior**, not afterwards:

```bash
git-cliff -o CHANGELOG.md
```

## Commands

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt
```

## Useful facts (verified on this machine)

```bash
pacman -Qqen              # explicitly installed, from repos
pacman -Qqem              # foreign / AUR
expac -S '%r' <pkg>       # repo name — can be third-party, e.g. cachyos-extra-v3
pacman -Qoq $(command -v <cmd>)   # binary → owning package
pacman -F <cmd>           # package providing a not-yet-installed command (needs pacman -Fy)
fc-match "<font>"         # silently falls back if the font is missing — compare the returned family
```

Repo names are machine-specific, package names are portable. `dotfiles.toml` stores
package names only.
