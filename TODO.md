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
| **M5** | **`external` mode** — alongside chezmoi/stow | 1 | M0 · M2 for `collect --external` |
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

- [ ] **Where `collect` writes** — is `~/dotfiles/` the default, or is `--out` mandatory? (M2)
- [ ] **A half-finished collect wizard** — save state for `--resume`, or start over? (M6)
- [ ] **`/` search** — filter the list, or jump to the match? (M6)
- [ ] **Bundle deletion confirmation** — `y/n`, or type the name? (M6)
- [ ] **Terminal under 80x24** — warn and quit, or compress the layout? (M6)

Answered, recorded here so they stop being re-asked:

**The four that shaped M1:**

- **Package installation fails during `use`** → **finish the switch, report, do not roll
  back.** Rolling back needs a transactional ledger, which is the single largest piece of
  complexity M1 could take on — for a state that is already harmless. Packages are never
  removed, so "some packages missing, links correct" is a working switch with a gap, and
  re-running `use` closes it with `--needed`. The summary names every failure.
- **Local-path bundles** → **a symlink in the store.** Everything under `bundles/` is then
  a directory and `ls` / `use` / `rm` have one code path. An absolute path in `state.toml`
  adds a second resolution branch to every one of them, to save one symlink.
- **Deleting the active bundle** → **refuse, and say `use -` first.** Deactivating on the
  user's behalf is destruction they did not ask for.
- **Uncommitted changes on switch** → **warn, do not block.** It is their repo, and in
  symlink mode their edits are already in it — switching away loses nothing.

**Earlier:**

- **License** → GPL-3.0, `LICENSE` committed. The `MIT` in the doc examples is a
  *bundle's* own license field, not the project's.
- **`name` uniqueness** → it is the store directory name; `--as` renames on collision.
- **`version`** → bumped by hand, never by the tool. `sync` does not touch it.
- **git URLs in v1** → yes, `add`/`use` take them (M4).
- **`README.md`** → `collect` generates it, from the same renderer as `post` (M3).

Nothing above blocks M0 or M1 any more; the remaining five are M2 and M6 questions.
**This is the only list** — the per-document "Open Decisions" sections are gone, because
keeping five of them is how three of these came to be answered elsewhere without anyone
striking them out.

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

### Apply (the only writing module — `src/apply/`, see `design.md` §8)
- [ ] `apply/ledger.rs` — `state.toml`: `active`, `previous`, links, `mkdirs`, `hooks_ran`
- [ ] `apply/backup.rs` — adoption: a real file at the target → backup + ledger entry
- [ ] `apply/backup.rs` — restore the adopted backup when a link is removed for good
- [ ] `apply/links.rs` — place links (`config/` by depth rule, `home/`+`local/` per file)
- [ ] `apply/links.rs` — create intermediate dirs, record them, remove them when left empty
- [ ] `apply/links.rs` — link diff for switching: remove / repoint / add
- [ ] `apply/system.rs` — `fc-cache -f` when anything under `~/.local/share/fonts` changed
- [ ] `apply/system.rs` — services: `enable --now`, and `disable --now` on the way out
- [ ] `apply/system.rs` — WM reload
- [ ] `apply/mod.rs` — the sequences, and **nothing else**: `activate()` / `switch()` /
      `deactivate()` read as `design.md` §4.2 and `profiles.md` §3 do
- [ ] A package failing does **not** roll the switch back (Phase 0) — it is collected and
      named in the summary
- [ ] A local-path bundle is a **symlink in the store** (Phase 0), so `ls`/`use`/`rm` see
      a directory like any other
- [ ] `hooks_ran` is written to the ledger but stays empty — **hooks do not run until M4.**
      `example/` has none, so M1's fixture is unaffected; a bundle that has one gets it
      skipped, and the plan says so rather than pretending
- [ ] `use -` (previous bundle)
- [ ] `ls` — bundles in the store, which is active, detached count
- [ ] `rm <name>` — refuses while the bundle is active, says `use -` first (Phase 0)
- [ ] `sync` — detect detached links, report them, and re-link (backing up the foreign file)
- [ ] `refs.rs` is **not** needed here — M1 installs a bundle, it does not judge one

`sync`'s **write-back** half is deliberately not in M1: the moment a file enters the
bundle it has to pass the §6 content scan, and `scan/secrets.rs` is M2. Detect + re-link
covers the case that actually bites (GTK ate the link), and it needs no scanner.

**Done means:** `dotpack use example` on a temporary `HOME` places **every file and
package the bundle declares** — links at the right depth, packages installed, services
enabled — and after A → B → `use -` the filesystem is bit-for-bit identical to the start,
adopted backups and created directories included.

Not *"produces a working rice"*: `example/` is deliberately incomplete (no
`scripts/quickshell/`, so no bar and no launcher — `example/README.md` says which gaps and
why). Judging M1 on the rice booting would be judging it on bundle content it does not
own. The full-rice claim is the acceptance test above, and it needs M4 plus the missing
content.

---

## M2 — collect · 2 weekends

Stops the bundle from having to be hand-written.

### Scan (reads only)
- [ ] `scan/wm.rs` — WM detection + per-WM key tables
- [ ] `scan/refs.rs` — extractor 1: `source` / `include` / `@import`, **not WM-specific**
- [ ] `scan/refs.rs` — extractor 2: any `~/`, `$HOME/`, `$(dirname …)/` token, **anywhere on
      the line** — this is the one that finds nine of `example/`'s ten dangling references
- [ ] `scan/refs.rs` — classify each reference: in-bundle / addable / system path / dead
- [ ] `scan/deps.rs` — extract commands, strip the `uwsm app --` / `sh -c` wrappers
- [ ] `scan/deps.rs` — drop builtin and coreutils noise
- [ ] `scan/deps.rs` — attach a `reason` to every suggestion
- [ ] `scan/deps.rs` — `-Qoq` may return a **provider** (`noctalia-qs` for `quickshell`):
      offer the installable name, never conclude "no such package" from `pacman -Ss`
- [ ] `scan/fonts.rs` — `fc-match`, compare the returned family, warn if it fell back
- [ ] `scan/fonts.rs` — `-Qoq` → **`pacman -F <basename>`** → ship the files, in that order
- [ ] `scan/fonts.rs` — GTK theme / icons / cursor, same three steps
- [ ] `scan/secrets.rs` — deny-list (including shell history)
- [ ] `scan/secrets.rs` — content patterns, findings unticked by default
- [ ] `scan/secrets.rs` — the same scan runs against the **active bundle** for `ls`
- [ ] `apply/write.rs::write_bundle()` — the bundle directory + `dotfiles.toml`
      (`README.md` is added in M3, where its renderer is written)
- [ ] `ignore` applies **here and only here**, and the "matches nothing" warning is
      checked against the source tree, never against a bundle
- [ ] `sync` write-back — the deferred half of M1, now that `secrets.rs` exists
- [ ] collect must not walk into the active bundle through its own symlinks

**Done means:** `collect` run on this machine, with the same directories ticked, produces
a bundle whose `dotfiles.toml` **matches `example/`'s** — `ttf-cascadia-mono-nerd` and
`starship` as packages rather than urls, `quickshell` rather than the `quickshell-git`
provide, and `config/kitty/catppuccin.conf` present, which the hand-written version forgot.

The file trees will differ and that is correct: `example/` is hand-trimmed (no `fish/`, no
27 MB `quickshell/`) and those are *selections*, made in step 2 of the wizard. The manifest
is the part `collect` is responsible for.

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

The way the standard spreads. The reading half can be done at any point after M0; the
writing half is a flag on a command that does not exist until M2.

- [ ] `mode = "external"` — do not touch files, install packages, show roles
- [ ] The `managed_by` field (informational, the tool is not called)
- [ ] `collect --external` — generate only the manifest for an existing chezmoi/stow repo
      **(needs M2)**
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

---

## Deferred (`ponytail:` markers)

- **`copy` installs** — removed outright, not deferred. A copied install cannot be
  switched, cannot be synced and has no ledger, which makes it `git clone` + `cp -r`. If
  someone genuinely asks, it is a flag on `add`, not a mode in the manifest.
- **Fetching a component's `url`** — not deferred either; it is a thing we do not do. A
  font that matters ships in `local/share/fonts/`.
- **`import <repo>`** — generate a draft manifest from a foreign rice's installer. There are zero repos in the world containing `dotfiles.toml` today; but the adoption lever is `post` (M3), not this. Later.
- Template engine — machine-specific data (monitor name, DPI). In v1 the only remedy is `ignore`. Known ceiling.
- The `modes` field — `chmod 600` / `444`. Git only tracks the exec bit.
- `use --prune` — suggest packages no bundle wants anymore
- `#variant` — in-bundle theme variants (syntax reserved)
- `flatpak` / `cargo` / `npm` backends (fields reserved, ignored in v1)
- In-TUI install progress panel — if pacman's output turns out not to be enough
- `regex` — the secret scan starts with substring matching
- Bulk `pacman -Qlq` — if per-command `-Qoq` becomes measurably slow
- Non-Arch distros, secret encryption
