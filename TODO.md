# Work Plan

The design is done (`docs/`), there is no code. This file says in which order it gets
written.

**Ordering principle:** every milestone leaves behind **something usable on its own**. Not
a half-finished layer, a working tool. When M1 is done it has to produce value for a single
user (you), with no ecosystem.

**CLI first, TUI second** — not a scope cut, an order. Every command is a function the TUI
will call; drawing the face before the engine runs means doing the work twice.

Effort unit: **a weekend** (~8 hours). Rough, not optimistic.

---

## Milestones

| | What comes out | Effort | Depends on |
|---|---|---|---|
| **M0** | Manifest + skeleton (not usable alone) | 1 | — |
| **M1** | **Switching** — `use` / `use -` / `ls` against a hand-written bundle | 1 | M0 |
| **M2** | **collect** — you no longer hand-write the bundle | 2 | M1 |
| **M3** | **Author tools** — `post`, the generated README | 1.5 | M2 |
| **M4** | **Sharing** — `add github:…` | 2 | M1 |
| **M5** | **`external` mode** — alongside chezmoi/stow | 1 | M0 |
| **M6** | TUI | 3 | M1–M4 |
| **M7** | Release prep | 2 | all |

### The acceptance test

One sentence, and every milestone below is judged against it:

> On a clean Arch machine, `dotpack use github:<this-repo>/example` produces the same
> working rice as `bash -c "$(curl -fsSL …/imperative-dots/master/install.sh)"`, without
> running one line of that script.

It is not reached at any single milestone — **M1** places the files and packages, **M4**
handles the hooks that cover what a manifest may not do (root services, `chsh`). What the
bundle still needs to contain for it to pass is listed in `example/README.md`.

~13.5 weekends in total. **The tool works at the end of M1**, one weekend in — because
`example/` is already a valid hand-written bundle and makes a perfect fixture. The old
plan bundled `collect` into M1 and pushed "something that runs" three weekends out; that
was backwards for a plan whose stated principle is that every milestone leaves a usable
tool behind.

---

## Phase 0 — Answer before writing code

- [x] **License** — GPL-3.0, `LICENSE` is committed. (The `MIT` in the doc examples is a
      *bundle's* own license field, not the project's.)
- [ ] **If package installation blows up during `use`** — finish the switch or roll it back?
      (Rolling back requires the ledger to be transactional; it changes M1's shape.)
- [ ] **Local-path bundles** — a symlink in the store, or an absolute path in `state.toml`?
- [ ] **When deleting the active bundle** — is `use -` required first, or is it deactivated automatically?
- [ ] **Switching while the active bundle's git has uncommitted changes** — warn or block?
- [ ] **Where `collect` writes** — is `~/dotfiles/` the default, or is `--out` mandatory? (M2)
- [ ] **A half-finished collect wizard** — save state for `--resume`, or start over? (M6)
- [ ] **`/` search** — filter the list, or jump to the match? (M6)
- [ ] **Bundle deletion confirmation** — `y/n`, or type the name? (M6)
- [ ] **Terminal under 80x24** — warn and quit, or compress the layout? (M6)

Answered, recorded here so they stop being re-asked:

- **License** → GPL-3.0.
- **`name` uniqueness** → it is the store directory name; `--as` renames on collision.
- **`version`** → bumped by hand, never by the tool. `sync` does not touch it.
- **git URLs in v1** → yes, `add`/`use` take them (M4).
- **`README.md`** → `collect` generates it, from the same renderer as `post` (M3).

The first four are the ones that shape M1. **This is the only list** — the per-document
"Open Decisions" sections are gone, because keeping five of them is how three of these
came to be answered elsewhere without anyone striking them out.

---

## M0 — Manifest + Skeleton · 1 weekend

Not usable alone; everything depends on it.

- [ ] `cargo init --bin`, edition 2024, 7 crates (`CLAUDE.md`)
- [ ] `paths.rs` — HOME / store / state / backups, the **only** place `env::var("HOME")` appears
- [ ] `manifest.rs` — `dotfiles.toml` serde types, read/write
- [ ] `manifest.rs` — `components`, short and long (inline table) forms
- [ ] `manifest.rs` — validation: hard error / warning split (`docs/manifest.md`)
- [ ] `manifest.rs` — warn if a `components[].pkg` is missing from `packages`
- [ ] `bundle.rs` — path mapping (`config/` → `~/.config`, `home/`, `local/`, `assets/`)
- [ ] `bundle.rs` — **the link depth rule** for `config/`: stop at the first directory containing files
- [ ] `bundle.rs` — `home/` and `local/` link **per file**, no depth rule
- [ ] `bundle.rs` — reject hook/asset paths escaping the bundle root
- [ ] `main.rs` — clap subcommands (empty bodies)
- [ ] The default ignore list, embedded

**Done means:** a hand-written `dotfiles.toml` survives a read → write round trip
unchanged; every invalid example in `docs/manifest.md` is rejected at the right severity.

---

## M1 — Switching · 1 weekend

**After this the tool works.** `example/` is the fixture: a hand-written bundle already in
the repo. No `collect`, no ecosystem, nobody else's repo required.

### Package layer
- [ ] `pkg.rs` — installed list via `pacman -Qqen` / `-Qqem`
- [ ] `pkg.rs` — binary → package: `pacman -Qoq $(command -v X)`
- [ ] `pkg.rs` — command not installed: `pacman -F`, detect that `-Fy` is needed
- [ ] `pkg.rs` — helper detection: `paru` → `yay` → `pikaur` → `trizen`
- [ ] `pkg.rs` — merge the `yay` + `paru` lists into a single AUR set
- [ ] `pkg.rs` — install: `pacman -S --needed`, then `<helper> -S --needed`
- [ ] Package not found → search the AUR → ask → otherwise report and continue
- [ ] **Never run `-Syu`**

### Apply (the only writing module)
- [ ] `apply.rs` — the `state.toml` link ledger: links, `mkdirs`, `hooks_ran`
- [ ] `apply.rs` — backup adoption: a real file at the target → backup + ledger entry
- [ ] `apply.rs` — place links (`config/` by depth rule, `home/`+`local/` per file)
- [ ] `apply.rs` — create intermediate dirs, record them, remove them when left empty
- [ ] `apply.rs` — link diff for switching: remove / repoint / add
- [ ] `apply.rs` — restore the adopted backup when a link is removed for good
- [ ] `apply.rs` — `fc-cache -f` when anything under `~/.local/share/fonts` changed
- [ ] `apply.rs` — services: `enable --now`, and `disable --now` on the way out
- [ ] `apply.rs` — WM reload
- [ ] `use -` (previous bundle)
- [ ] `ls` — bundles in the store, which is active, detached count
- [ ] `sync` — detect detached links, write back into the bundle, re-link
- [ ] `sync` — run the §6 content scan on anything it writes back

**Done means:** `dotpack use example` on a temporary `HOME` produces a working hyprland
setup, and after A → B → `use -` the filesystem is bit-for-bit identical to the start,
adopted backups and created directories included.

---

## M2 — collect · 2 weekends

Stops the bundle from having to be hand-written.

### Scan (reads only)
- [ ] `scan/wm.rs` — WM detection + per-WM key tables
- [ ] `scan/refs.rs` — `source` / `include` / `@import` resolution, **not WM-specific**
- [ ] `scan/refs.rs` — classify each reference: in-bundle / addable / system path / dead
- [ ] `scan/deps.rs` — extract commands, strip the `uwsm app --` / `sh -c` wrappers
- [ ] `scan/deps.rs` — drop builtin and coreutils noise
- [ ] `scan/deps.rs` — attach a `reason` to every suggestion
- [ ] `scan/fonts.rs` — `fc-match`, compare the returned family, warn if it fell back
- [ ] `scan/fonts.rs` — `-Qoq` → **`pacman -F <basename>`** → ship the files, in that order
- [ ] `scan/fonts.rs` — GTK theme / icons / cursor, same three steps
- [ ] `scan/secrets.rs` — deny-list (including shell history)
- [ ] `scan/secrets.rs` — content patterns, findings unticked by default
- [ ] `scan/secrets.rs` — the same scan runs against the **active bundle** for `ls`
- [ ] `apply.rs::write_bundle()` — the bundle directory + `dotfiles.toml` + `README.md`
- [ ] `ignore` applies **here and only here**
- [ ] collect must not walk into the active bundle through its own symlinks

**Done means:** `collect` run on this machine reproduces `example/` — including
`ttf-cascadia-mono-nerd` in `packages` rather than a `url`, and including
`config/kitty/catppuccin.conf`, which the hand-written version forgot.

---

## M3 — Author Tools · 1.5 weekends

The adoption lever. People already have to write this list by hand; if the tool produces
it, filling in the format becomes a gain rather than a chore.

- [ ] `scan/roles.rs` — package → role table (~40 lines), fill in `components`
- [ ] `post.rs` — `components` → a shareable list
- [ ] `post.rs` — `--format reddit|markdown|plain`
- [ ] `post.rs` — copy to clipboard (`wl-copy` / `xclip`, whichever exists)
- [ ] The same renderer writes `README.md` during `collect`

**Done means:** the `[components]` block in `docs/standard.md` produces that document's
list exactly.

---

## M4 — Sharing · 2 weekends

- [ ] Source resolution: `github:U/R[/branch]`, `gitlab:`, `https://`, local paths
- [ ] `git clone --depth 1` (real repos are 75 MB+)
- [ ] Parse `#variant`, say "not supported in v1"
- [ ] Reject a repo without `dotfiles.toml` with a clear message — do not run a foreign `install.sh`
- [ ] `requires` version check: strip epoch and pkgrel from `pacman -Q`, compare field by
      field **as integers** (`0.9` vs `0.56`) — warn, do not block
- [ ] `wm` mismatch — warn, do not block
- [ ] Validation-time reference check on a foreign bundle (`scan/refs.rs`, no machine state)
- [ ] Install plan: packages / files / services / hook / manual-step summary + confirmation
- [ ] Manual steps: every `components` entry carrying a `url` is printed, never fetched
- [ ] Hooks: **show the contents**, confirm, run with `DP_BUNDLE_DIR` / `DP_MODE`
- [ ] Hooks run once per bundle — check and update `hooks_ran` in the ledger
- [ ] `--yes`, `--no-hooks`, `--run-hooks`, `--as <name>`
- [ ] Restore the terminal during package installation, let pacman's output stream

**Done means:** `dotpack use github:<your-own-repo>` works on a clean user.

---

## M5 — `external` Mode · 1 weekend

The way the standard spreads. Can be done at any point after M0.

- [ ] `mode = "external"` — do not touch files, install packages, show roles
- [ ] The `managed_by` field (informational, the tool is not called)
- [ ] `collect --external` — generate only the manifest for an existing chezmoi/stow repo
- [ ] A clear warning in the install summary: "you will place the files with `chezmoi apply`"

**Done means:** a single `dotfiles.toml` added to Brozi's chezmoi repo installs the
packages via `dotpack use` and touches no files.

---

## M6 — TUI · 3 weekends

- [ ] Event loop, alternate screen, raw mode
- [ ] `std::panic::set_hook` restores the terminal first
- [ ] Leave/re-enter the terminal around package installation
- [ ] Worker thread + `mpsc` — scanning must not freeze the UI
- [ ] Main screen: bundle list, active marker, detached + secret counters, detail panel
- [ ] Switch plan screen, hook source with `h`
- [ ] Collect wizard, 5 steps — warnings screen carries secrets **and** dangling references
- [ ] Checklist widget (ratatui has none built in — `List` + our own state)
- [ ] Consistent keymap + `?` help
- [ ] Terminal palette colors only

**Done means:** every screen in `docs/tui.md` is reachable, and `esc` goes back everywhere
without applying anything.

---

## M7 — Release Prep · 2 weekends

- [ ] Test on sway and i3 (not just hyprland)
- [ ] Test on a clean user with no helper installed
- [ ] Publish an example bundle repo — the reference people will look at
- [ ] Publish the spec as a document separate from the tool (`docs/standard.md` + `manifest.md`)
- [ ] License file

---

## Deferred (`ponytail:` markers)

- **`copy` installs** — removed outright, not deferred. A copied install cannot be
  switched, cannot be synced and has no ledger, which makes it `git clone` + `cp -r`. If
  someone genuinely asks, it is a flag on `add`, not a mode in the manifest.
- **Fetching a component's `url`** — not deferred either; it is a thing we do not do. A
  font that matters ships in `local/share/fonts/`.
- **`import <repo>`** — generate a draft manifest from a foreign rice's installer. There are zero repos in the world containing `dotfiles.toml` today; but the adoption lever is `post` (M2), not this. Later.
- Template engine — machine-specific data (monitor name, DPI). In v1 the only remedy is `ignore`. Known ceiling.
- The `modes` field — `chmod 600` / `444`. Git only tracks the exec bit.
- `use --prune` — suggest packages no bundle wants anymore
- `#variant` — in-bundle theme variants (syntax reserved)
- `flatpak` / `cargo` / `npm` backends (fields reserved, ignored in v1)
- In-TUI install progress panel — if pacman's output turns out not to be enough
- `regex` — the secret scan starts with substring matching
- Bulk `pacman -Qlq` — if per-command `-Qoq` becomes measurably slow
- Non-Arch distros, secret encryption
