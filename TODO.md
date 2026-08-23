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
| **M1** | **Personal rice switching** — a working tool | 3 | M0 |
| **M2** | **Author tools** — `post`, the generated README | 1.5 | M1 |
| **M3** | **Sharing** — `add github:…`, `install` | 2 | M1 |
| **M4** | **`external` mode** — alongside chezmoi/stow | 1 | M0 |
| **M5** | TUI | 3 | M1–M3 |
| **M6** | Release prep | 2 | all |

~13.5 weekends in total. You can stop wherever you like after M1 — the tool works by then.

---

## Phase 0 — Answer before writing code

- [ ] **License** — needed if this goes public. The examples say MIT, confirm it.
- [ ] **Where `collect` writes** — is `~/dotfiles/` the default, or is `--out` mandatory?
- [ ] **If package installation blows up during `use`** — finish the switch or roll it back?
      (Rolling back requires the ledger to be transactional; it changes M1's shape.)
- [ ] **Local-path bundles** — a symlink in the store, or an absolute path in `state.toml`?
- [ ] **When deleting the active bundle** — is `use -` required first, or is it deactivated automatically?
- [ ] **The `version` field** — bumped by hand, or bumped automatically by `sync`?
- [ ] **Switching while the active bundle's git has uncommitted changes** — warn or block?

The first three affect M1 directly, the rest can be answered later.

---

## M0 — Manifest + Skeleton · 1 weekend

Not usable alone; everything depends on it.

- [ ] `cargo init --bin`, edition 2024, 7 crates (`CLAUDE.md`)
- [ ] `manifest.rs` — `dotfiles.toml` serde types, read/write
- [ ] `manifest.rs` — `components`, short and long (inline table) forms
- [ ] `manifest.rs` — validation: hard error / warning split (`docs/manifest.md`)
- [ ] `manifest.rs` — warn if a `components[].pkg` is missing from `packages`
- [ ] `bundle.rs` — path mapping (`config/` → `~/.config`, `home/`, `local/`, `assets/`)
- [ ] `bundle.rs` — **the link depth rule**: stop at the first directory containing files
- [ ] `bundle.rs` — reject hook/asset paths escaping the bundle root
- [ ] `main.rs` — clap subcommands (empty bodies)
- [ ] The default ignore list, embedded

**Done means:** a hand-written `dotfiles.toml` survives a read → write round trip
unchanged; every invalid example in `docs/manifest.md` is rejected at the right severity.

---

## M1 — Personal Rice Switching · 3 weekends

**After this the tool works.** No ecosystem, no adoption, nobody else's repo required. This
is the most defensible core.

### Package layer
- [ ] `pkg.rs` — installed list via `pacman -Qqen` / `-Qqem`
- [ ] `pkg.rs` — binary → package: `pacman -Qoq $(command -v X)`
- [ ] `pkg.rs` — command not installed: `pacman -F`, detect that `-Fy` is needed
- [ ] `pkg.rs` — helper detection: `paru` → `yay` → `pikaur` → `trizen`
- [ ] `pkg.rs` — merge the `yay` + `paru` lists into a single AUR set
- [ ] `pkg.rs` — install: `pacman -S --needed`, then `<helper> -S --needed`
- [ ] Package not found → search the AUR → ask → otherwise report and continue
- [ ] **Never run `-Syu`**

### Collect (reads only)
- [ ] `scan/wm.rs` — WM detection + per-WM key tables
- [ ] `scan/wm.rs` — follow `source` / `include` chains
- [ ] `scan/deps.rs` — extract commands, strip the `uwsm app --` / `sh -c` wrappers
- [ ] `scan/deps.rs` — drop builtin and coreutils noise
- [ ] `scan/deps.rs` — attach a `reason` to every suggestion
- [ ] `scan/secrets.rs` — deny-list (including shell history)
- [ ] `scan/secrets.rs` — content patterns, findings unticked by default
- [ ] `collect` — write the bundle directory and `dotfiles.toml`

### Apply (the only writing module)
- [ ] `apply.rs` — the `state.toml` link ledger
- [ ] `apply.rs` — backup adoption: a real file at the target → backup + ledger entry
- [ ] `apply.rs` — place links (per the depth rule)
- [ ] `apply.rs` — `copy` mode, **preserving mode bits** (`fs::copy`)
- [ ] `apply.rs` — link diff for switching: remove / repoint / add
- [ ] `apply.rs` — restore the adopted backup when a link is removed for good
- [ ] `apply.rs` — services: `systemctl --user`
- [ ] `apply.rs` — WM reload
- [ ] `use -` (previous bundle)
- [ ] `ls` — bundles in the store, which is active, dirty file count
- [ ] `sync` — detect broken symlinks, offer to write back

**Done means:** after A → B → `use -` the filesystem is bit-for-bit identical to the start,
adopted backups included. Write the test with a temporary `HOME`.

---

## M2 — Author Tools · 1.5 weekends

The adoption lever. People already have to write this list by hand; if the tool produces
it, filling in the format becomes a gain rather than a chore.

- [ ] `scan/roles.rs` — package → role table (~40 lines), fill in `components`
- [ ] `scan/fonts.rs` — `fc-match`, warn "font missing" if the returned family does not match
- [ ] `scan/fonts.rs` — GTK theme / icons / cursor → `/usr/share/{themes,icons}` → package
- [ ] `scan/fonts.rs` — no owning package (hand-installed under `~/.local/share`) → warn and
      ship the files, do not drop the component (`docs/real-world.md` F17)
- [ ] `post.rs` — `components` → a shareable list
- [ ] `post.rs` — `--format reddit|markdown|plain`
- [ ] `post.rs` — copy to clipboard (`wl-copy` / `xclip`, whichever exists)
- [ ] The same renderer writes `README.md` during `collect`

**Done means:** the `[components]` block in `docs/standard.md` produces that document's
list exactly.

---

## M3 — Sharing · 2 weekends

- [ ] Source resolution: `github:U/R[/branch]`, `gitlab:`, `https://`, local paths
- [ ] `git clone --depth 1` (real repos are 75 MB+)
- [ ] Parse `#variant`, say "not supported in v1"
- [ ] Reject a repo without `dotfiles.toml` with a clear message — do not run a foreign `install.sh`
- [ ] `requires` version check (`pacman -Q`) — warn, do not block
- [ ] `wm` mismatch — warn, do not block
- [ ] Install plan: packages / files / services / hook summary + confirmation
- [ ] Hooks: **show the contents**, confirm, run with `DP_BUNDLE_DIR` / `DP_MODE`
- [ ] `--yes`, `--no-hooks`, `--copy`, `--symlink`, `--as <name>`
- [ ] Restore the terminal during package installation, let pacman's output stream

**Done means:** `dotpack use github:<your-own-repo>` works on a clean user.

---

## M4 — `external` Mode · 1 weekend

The way the standard spreads. Can be done at any point after M0.

- [ ] `mode = "external"` — do not touch files, install packages, show roles
- [ ] The `managed_by` field (informational, the tool is not called)
- [ ] `collect --external` — generate only the manifest for an existing chezmoi/stow repo
- [ ] A clear warning in the install summary: "you will place the files with `chezmoi apply`"

**Done means:** a single `dotfiles.toml` added to Brozi's chezmoi repo installs the
packages via `dotpack use` and touches no files.

---

## M5 — TUI · 3 weekends

- [ ] Event loop, alternate screen, raw mode
- [ ] `std::panic::set_hook` restores the terminal first
- [ ] Leave/re-enter the terminal around package installation
- [ ] Worker thread + `mpsc` — scanning must not freeze the UI
- [ ] Main screen: bundle list, active marker, dirty counter, detail panel
- [ ] Switch plan screen, hook source with `h`
- [ ] Collect wizard, 5 steps
- [ ] Checklist widget (ratatui has none built in — `List` + our own state)
- [ ] Consistent keymap + `?` help
- [ ] Terminal palette colors only

**Done means:** every screen in `docs/tui.md` is reachable, and `esc` goes back everywhere
without applying anything.

---

## M6 — Release Prep · 2 weekends

- [ ] Test on sway and i3 (not just hyprland)
- [ ] Test on a clean user with no helper installed
- [ ] Publish an example bundle repo — the reference people will look at
- [ ] Publish the spec as a document separate from the tool (`docs/standard.md` + `manifest.md`)
- [ ] License file

---

## Deferred (`ponytail:` markers)

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
