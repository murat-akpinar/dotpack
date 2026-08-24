# CLAUDE.md

Project instructions for Claude Code working in this repo.

## What this is

`dotpack` bundles dotfiles together with the packages they need, installs
them in one command, and switches between rices by re-pointing symlinks.

Rust + ratatui · Arch only · hyprland / sway / i3.

**M0–M6 are done: switching, `collect`, `post` + the generated README, sharing —
`use github:user/repo` clones, plans, warns and runs the bundle's hooks — `external` mode,
and the TUI.** `dotpack` with no arguments opens `src/tui/`; every screen calls the same
function the CLI calls. **M7 is done too**: sway, i3 and a helper-less machine were tested
on a second box, the format lives in `spec/` on its own, the example bundle is published at
`murat-akpinar/dotpack-example`, and `assets` — specified since M0, implemented nowhere —
are now copied. **`spec/` is the format and is normative; `docs/` is the
tool's own thinking.** Read `docs/design.md` before proposing any structure, and
`docs/real-world.md` for what real shared rices actually look like (several design rules
come from that teardown).
`TODO.md` is the running score: each finished milestone carries a note on what it cost and
what it got wrong.

**Everything in this repo is written in English** — docs, filenames, code, comments,
commit messages. The user writes in Turkish; the files do not.

## Terminology

- **bundle** — a directory in the shared format (`dotfiles.toml` + `config/` + …). One git repo = one bundle.
- **store** — `~/.local/share/dotpack/bundles/`, where bundles live locally.
- **active** — the one bundle whose symlinks are currently in place. Exactly one, never partial.
- **external mode** — `mode = "external"`: the bundle ships no files, only `packages` + `components`. Files stay under chezmoi/stow. This is a v1 feature and the format's main distribution path, not a fallback.
- **components** — optional role→component map in `dotfiles.toml` (`bar`, `terminal`, `icons`…). Descriptive only; install logic reads `packages`. This is the standard the project is trying to set — see `spec/components.md`.
- **link ledger** — `~/.local/state/dotpack/state.toml`, the **only** state file: active and previous bundle, every link placed, every directory created to place it, and which bundles' hooks have already run. Switching cleanly depends entirely on this file being accurate.
- **detached link** — an application deleted our symlink and wrote a real file in its place (GTK, VS Code). The only thing `sync` exists for.

## Invariants — do not violate

1. **`scan/` never writes to disk.** It reads and returns suggestions. `src/apply/` is
   the only place that mutates the filesystem — **including `collect`'s output**
   (`apply::write::write_bundle()`). A `collect.rs` that writes its own files makes this
   invariant false on day one. The rule is greppable, so keep it that way:
   `grep -rlE 'fs::(write|copy|create_dir|remove|rename)|os::unix::fs::symlink|OpenOptions' src/ | grep -v '^src/apply/'`
   must print nothing. (Bare `symlink` in the pattern also matches `symlink_metadata()`,
   which *reads* — `scan/refs.rs` needs it, and a check that always prints something is a
   check nobody runs.)
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
11. **Nothing is ever fetched from a `url` in a manifest.** A `url` in `components` means
   "the user does this by hand" and is printed in the summary. Fonts and themes that no
   package provides ship **inside the bundle** under `local/share/`. See `docs/design.md`
   §5.2 and `real-world.md` F5.
12. **`ignore` is collect-time only.** It keeps a path out of the bundle. It cannot skip a
   file at install time — `~/.config/hypr` is one directory link. Anything that `source`s
   an ignored file breaks; §5.1 reports it.
13. **Hooks run on a bundle's first activation only**, recorded in the ledger. Real hooks
   append to files; appending twice is not undoable. `pre_install` runs **before packages**,
   `post_install` **after links and services** — one ordering, first activation and switch
   alike.
14. **`HOME` is read in `paths.rs` and nowhere else.** M1's acceptance test runs against a
   temporary `HOME`; that is impossible if `env::var` is scattered.

## Link rules

**`config/` — directory link, at the depth rule's depth.** Walk down from `config/`. If a
directory contains files directly, link it and stop. If it only contains directories,
descend again.

- `config/hypr/hyprland.conf` → link `~/.config/hypr`
- `config/hypr/themes/x/*` (nothing above it) → link `~/.config/hypr/themes/x`

The second case is real: rices install *alongside* a user's own config, not over it.

**`home/` and `local/` — per file, always.** `~`, `~/.local/bin` and
`~/.local/share/fonts` are mixed directories holding the user's own things; a directory
link hides them. Hand-installed Nerd Fonts live in exactly that path (`real-world.md` F17).

Every directory created to place a link goes in the ledger and is removed on deactivation
**if empty**. After anything lands under `~/.local/share/fonts`, run `fc-cache -f`.

**`assets/` — copied, not linked, and not in the ledger.** They go where `dotfiles.toml`
says, on every activation, over nothing already at the dest, and nothing removes them
again. `~/Pictures/wallpapers` is the user's directory, so unlike a link target it is
never backed up or adopted.

## Reference integrity

Every shipped text file is scanned for references and each one is resolved. A reference
pointing outside the bundle is reported — at collect time to the author, at validation
time to the receiver. This is **not** WM-specific: `kitty.conf`'s
`include ~/.config/kitty/catppuccin.conf` is the case that motivated it, and shipping one
without the other installs a kitty that errors on every start.

**Two extractors, and the second is the one that finds most of them.** Keywords
(`source` / `include` / `@import`) catch `include catppuccin.conf`. But the example
bundle's other eleven dangling references are none of them — they are **paths sitting in
ordinary argument position**:

```
exec-once = swayosd-server --style "$HOME/.config/swayosd/style.css"
exec-once = quickshell -p ~/.config/hypr/scripts/quickshell/Shell.qml
SCRIPTS_DIR="$HOME/.config/hypr/scripts/quickshell"
```

So: any token beginning `~/`, `$HOME/`, or `$(dirname …)/` in a shipped text file is a
reference. `docs/design.md` §5.1.

## Dependencies

Current set — `ratatui`, `serde`, `toml`, `clap`, `walkdir`, `anyhow`. **Six, not seven:**
crossterm arrives as `ratatui::crossterm`, so there is one re-export instead of two version
numbers that have to agree. ratatui is pulled with `default-features = false` +
`features = ["crossterm"]` — the default set drags in widgets nothing draws.

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

**A new shelled-out binary is a `depends` line in `PKGBUILD`.** The crates are vendored by
cargo and need nothing there; `git` and `fc-cache` are called by name and do. Optional ones
(`paru`, `wl-copy`) are `optdepends` — the tool works without them and says so.

## Style

- Follow the ladder: does it need to exist → stdlib → already-installed dep → one line → minimum code.
- Mark deliberate shortcuts with a `ponytail:` comment naming the ceiling and the upgrade path. Several are already recorded in the docs — carry them into the code.
- Non-trivial logic leaves one runnable check behind (`#[test]` next to the code). No test frameworks, no fixtures.
- **Comments mark sections, not lines.** In config, manifest and data files use block
  markers and let them carry the structure:

  ```toml
  # --- packages start ---
  ...
  # --- packages end ---
  ```

  A per-line comment is for the one thing a reader would otherwise get *wrong* — a value
  that looks like a mistake but isn't, a deliberate omission. Rationale, history and
  "why we changed our mind" go in the README next to the file, never in the file. The
  same holds in Rust: section markers over a comment per statement.
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
pacman -Qoq $(command -v <cmd>)   # binary → owning package. May return a name that is NOT
                          # the command: /usr/bin/quickshell is owned by `noctalia-qs`.
expac -Q '%S' <pkg>       # ...because noctalia-qs PROVIDES quickshell and quickshell-git.
                          # `pacman -Ss '^quickshell-git$'` finds nothing — -Ss does not
                          # search provides. Never conclude "no such package" from -Ss.
pacman -F <cmd>           # package providing a not-yet-installed command
pacman -F <font.ttf>      # ...and the same trick by BASENAME finds the package that ships a
                          # hand-installed font, where -Qoq says "no owner".
                          # ⚠ REASONED, NOT RUN: needs `pacman -Fy` and there is no .files db
                          # on this machine. The conclusion was cross-checked another way
                          # (`pacman -Si ttf-cascadia-mono-nerd` → extra, 3.5.1-1). What proves
                          # -F is necessary rather than convenient: `pacman -Ss <basename>`
                          # returns nothing at all.
fc-match "<font>" --format '%{family}\n%{file}\n'
                          # silently falls back if the font is missing — compare the returned family
fc-cache -f               # a font nothing has indexed does not exist to running apps
```

Repo names are machine-specific, package names are portable. `dotfiles.toml` stores
package names only.
