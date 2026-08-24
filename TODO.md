# Work Plan

The design is done (`docs/`). This file says in which order the code gets written, and
what each milestone actually turned out to be worth. **M0–M6 are done**; M7 (release prep)
is what is left.

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
bundle still needs to contain for it to pass is listed in `example/README.md`, and after
M4 the tool's own half is done: what is left is bundle content.

~13.5 weekends in total. **The tool works at the end of M1**, one weekend in — because
`example/` is already a valid hand-written bundle and makes a perfect fixture. The old
plan bundled `collect` into M1 and pushed "something that runs" three weekends out; that
was backwards for a plan whose stated principle is that every milestone leaves a usable
tool behind.

---

## Phase 0 — Answer before writing code

- [ ] **When are `assets` copied** — first activation only, or every switch? `design.md`
      §4.2's thirteen steps have no asset step at all, and no bundle in the repo has one.
      `bundle::assets()` maps the paths and nothing calls it (M1)


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

- **`use -` with no previous bundle** → **deactivate.** The state before the first
  activation was "nothing placed", so that is where going back has to land. It also gives
  the only way out of `rm`'s refusal without inventing a verb the design's table does not
  have.

- **Where `collect` writes** → **`~/dotfiles` by default**, `--out` overrides, a
  non-empty directory is refused. The TUI's 1/5 screen has to prefill an output path
  anyway, so the default exists either way; making the CLI reject it only puts the same
  default in two places. The refusal points at `collect --external`, because a
  non-empty `~/dotfiles` usually means chezmoi or stow already lives there.
- **What `collect` selects with no arguments** → **the pre-ticked set**: the WM's own
  directory plus the ones its config starts or points at. Arguments override it. Making
  arguments mandatory would leave the wizard's "WM-related ones pre-ticked" (§4.1 screen
  2/5) with no caller until M6, which is the doing-it-twice the plan exists to avoid.
- **Shipping a theme or an icon set** → **no.** Fonts ship (they are small, and a missing
  Nerd Font turns every icon in the bar into a box); a theme or cursor that no package
  provides becomes a `components` entry with its path and a warning. Papirus is 100 MB,
  and `example/`'s own cursor is a manual step for exactly this reason.

**The four M6 asked:**

- **A half-finished collect wizard** → **start over.** `--resume` needs a second state
  file and `state.toml` is the only one there is; the scan it would restore is one
  keypress away.
- **`/` search** → **filter the list.** On a screen where every row is a decision, hiding
  the rest is the point. Ticks are stored by name, so a filtered list cannot tick the
  wrong row — which is the failure a jump-to-match design does not have and is the only
  reason to prefer it.
- **Bundle deletion confirmation** → **`y/n`.** `rm` refuses while the bundle is active, a
  local bundle is only a link in the store, and a cloned one clones again. Typing the name
  belongs to losses that cannot be undone; this is not one.
- **Terminal under 80x24** → **neither warn-and-quit nor a compressed layout.** One line
  saying what is missing, and the loop keeps running: a resize redraws and nothing in
  progress is lost. Quitting on a window drag is hostile, and a second layout is a second
  layout to maintain for the case nobody stays in.

**Earlier:**

- **License** → GPL-3.0, `LICENSE` committed. The `MIT` in the doc examples is a
  *bundle's* own license field, not the project's.
- **`name` uniqueness** → it is the store directory name; `--as` renames on collision.
- **`version`** → bumped by hand, never by the tool. `sync` does not touch it.
- **git URLs in v1** → yes, `add`/`use` take them (M4).
- **`README.md`** → `collect` generates it, from the same renderer as `post` (M3).

One question is left open — when `assets` are copied — and it is M1's, still waiting for a
bundle that has any.
**This is the only list** — the per-document "Open Decisions" sections are gone, because
keeping five of them is how three of these came to be answered elsewhere without anyone
striking them out.

---

## M0 — Manifest + Skeleton · 1 weekend

Not usable alone; everything depends on it.

- [x] `cargo init --bin`, edition 2024 — **5 crates**: `ratatui` + `crossterm` are not
      imported by anything until M6, so they are added there
- [x] `paths.rs` — HOME / store / state / backups, the **only** place `env::var("HOME")` appears
- [x] `manifest.rs` — `dotfiles.toml` serde types, read/write
- [x] `manifest.rs` — `components`, short and long (inline table) forms
- [x] `manifest.rs` — validation: hard error / warning split (`docs/manifest.md`)
- [x] `manifest.rs` — warn if a `components[].pkg` is missing from `packages`
- [x] `bundle.rs` — path mapping (`config/` → `~/.config`, `home/`, `local/`, `assets/`)
- [x] `bundle.rs` — **the link depth rule** for `config/`: stop at the first directory containing files
- [x] `bundle.rs` — `home/` and `local/` link **per file**, no depth rule
- [x] `bundle.rs` — reject hook/asset paths escaping the bundle root
- [x] `main.rs` — clap subcommands (empty bodies)
- [x] The default ignore list, embedded

**Done means:** a hand-written `dotfiles.toml` survives a read → write round trip
unchanged; every invalid example in `docs/manifest.md` is rejected at the right severity.

✅ Done. `example/dotfiles.toml` round-trips, the depth rule collapses its 34 files into 3
directory links, and `cargo test` covers both. Comments are not preserved by the round
trip and do not need to be — the tool only ever *writes* a manifest it generated itself
(`apply::write`, M2); a hand-written file is read, never rewritten.

---

## M1 — Switching · 1 weekend

**After this the tool works.** `example/` is the fixture: a hand-written bundle already in
the repo. No `collect`, no ecosystem, nobody else's repo required.

### Package layer
- [x] `pkg.rs` — what is missing: **`pacman -T`**, not a set difference against `-Qqen` /
      `-Qqem`. `-T` resolves *provides*, so the installed `noctalia-qs` satisfies a bundle
      asking for `quickshell`; subtracting `-Qq` calls it missing and `-S` then hits a
      conflict. One call, and the same call afterwards names every failure exactly
- [x] `pkg.rs` — helper detection: `paru` → `yay` → `pikaur` → `trizen`
- [x] `pkg.rs` — merge the `yay` + `paru` lists into a single AUR set
- [x] `pkg.rs` — install: `pacman -S --needed`, then `<helper> -S --needed`
- [x] Package not found → handed to the AUR helper, which searches both. One unknown name
      would otherwise make `pacman -S` refuse the **whole** transaction. With no helper it
      is reported and the switch continues
- [x] **Never run `-Syu`** — a test greps for it
- → **moved to M2**: `-Qqen` / `-Qqem`, `-Qoq $(command -v X)` and `pacman -F` / `-Fy`.
      They are the command → package chain's primitives and `scan/deps.rs` is their only
      caller; written here they would be untested code with nothing calling it

### Apply (the only writing module — `src/apply/`, see `design.md` §8)
- [x] `apply/ledger.rs` — `state.toml`: `active`, `previous`, links, `mkdirs`, `hooks_ran`
- [x] `apply/backup.rs` — adoption: a real file at the target → backup + ledger entry
- [x] `apply/backup.rs` — restore the adopted backup when a link is removed for good
- [x] `apply/links.rs` — place links (`config/` by depth rule, `home/`+`local/` per file)
- [x] `apply/links.rs` — create intermediate dirs, record them, remove them when left empty
- [x] `apply/links.rs` — link diff for switching: remove / repoint / add
- [x] `apply/system.rs` — `fc-cache -f` when anything under `~/.local/share/fonts` changed
- [x] `apply/system.rs` — services: `enable --now`, and `disable --now` on the way out
- [x] `apply/system.rs` — WM reload
- [x] `apply/mod.rs` — the sequences, and **nothing else**
- [x] A package failing does **not** roll the switch back (Phase 0) — `pacman -T` runs again
      after the install and whatever it still reports is the failure list, named
- [x] A local-path bundle is a **symlink in the store** (Phase 0)
- [x] `hooks_ran` is written to the ledger but stays empty — **hooks do not run until M4.**
      A bundle that declares one is told so in the plan and in the summary
- [x] `use -` (previous bundle), and with no previous bundle it deactivates
- [x] `ls` — bundles in the store, which is active, detached count (the secret count needs
      `scan/secrets.rs`, M2)
- [x] `rm <name>` — refuses while the bundle is active, says `use -` first (Phase 0)
- [x] `sync` — detect detached links, report them, and re-link (backing up the foreign file)
- [x] `refs.rs` is **not** needed here — M1 installs a bundle, it does not judge one

`sync`'s **write-back** half is deliberately not in M1: the moment a file enters the
bundle it has to pass the §6 content scan, and `scan/secrets.rs` is M2. Detect + re-link
covers the case that actually bites (GTK ate the link), and it needs no scanner.

**Done means:** `dotpack use example` on a temporary `HOME` places **every file and
package the bundle declares** — links at the right depth, packages installed, services
enabled — and after A → B → `use -` the filesystem is bit-for-bit identical to the start,
adopted backups and created directories included.

✅ Done — `tests/switch.rs`, which drives the real binary against a temporary `HOME` with
`pacman`, `sudo`, `systemctl`, `fc-cache`, `hyprctl` and all four AUR helpers stubbed on
`PATH`. Package installation is therefore *planned* in the test, not performed: a test
that installs 76 packages and enables easyeffects on the machine running it is not a test.
The plan itself was checked against the real `pacman -T` on this machine — it reports
exactly `starship` and `ttf-cascadia-mono-nerd` missing, which are precisely the two the
docs describe as hand-installed here.

Not *"produces a working rice"*: `example/` is deliberately incomplete (no
`scripts/quickshell/`, so no bar and no launcher — `example/README.md` says which gaps and
why). Judging M1 on the rice booting would be judging it on bundle content it does not
own. The full-rice claim is the acceptance test above, and it needs M4 plus the missing
content.

---

## M2 — collect · 2 weekends

Stops the bundle from having to be hand-written.

### Scan (reads only)
- [x] `scan/wm.rs` — WM detection + per-WM key tables
- [x] `scan/refs.rs` — extractor 1: `source` / `include` / `@import`, **not WM-specific**
- [x] `scan/refs.rs` — extractor 2: any `~/`, `$HOME/`, `$(dirname …)/` token, **anywhere on
      the line** — over `example/` it extracts 82 references against the keyword table's 6
- [x] `scan/refs.rs` — classify each reference: shipped / addable / system path / dead /
      unresolved, with the three exclusions. How many survive classification on a real
      machine is checked when `collect` wires it up — that needs a HOME whose `.config`
      *is* the rice
- [x] `scan/deps.rs` — extract commands, strip the `uwsm app --` / `sh -c` wrappers, and
      split pipelines: `grim … | satty` is two dependencies
- [x] `scan/deps.rs` — drop noise **by owning package** (`coreutils`, `systemd`, …) rather
      than by a list of command names. Shorter and more accurate: it drops `sleep` and
      `pkill` without naming them and keeps `notify-send`, which a hand-written list eats
- [x] `scan/deps.rs` — attach a `reason` to every suggestion
- [x] `scan/deps.rs` — `-Qoq` may return a **provider** (`noctalia-qs` for `quickshell`):
      offer the installable name, never conclude "no such package" from `pacman -Ss`
- [x] `pkg.rs` — the primitives moved here from M1: `-Qoq`, `-Qqem`, `pacman -F` with
      `-Fy` detection, and `which` without a shell. `/usr/local/bin/starship` resolves
      without the files database, because `pacman -Si starship` answers first
- [x] `scan/fonts.rs` — `fc-match`, compare the returned family, warn if it fell back.
      On this machine it earns its keep immediately: `settings.conf` asks for
      `CaskaydiaMono Nerd Font Mono SemiBold` and fontconfig quietly answers `Noto Sans Mono`
- [x] `scan/fonts.rs` — `-Qoq` → **`pacman -F <basename>`** → ship the files, in that order
- [x] `scan/fonts.rs` — GTK theme / icons / cursor, same three steps, keyed on
      `<dir>/index.theme` rather than the directory: `/usr/share/icons/Adwaita` has two
      owners (`adwaita-cursors` and `adwaita-icon-theme`), `index.theme` has one
- [x] `scan/secrets.rs` — deny-list (including shell history)
- [x] `scan/secrets.rs` — content patterns, matched at a token boundary so `disk-usage` is
      not an OpenAI key. Unticking by default is the collect wizard's job (M6); the
      scanner returns findings and decides nothing
- [x] `scan/secrets.rs` — the same scan runs against the **active bundle** for `ls`
- [x] `apply/write.rs::write_bundle()` — the bundle directory + `dotfiles.toml`
      (`README.md` is added in M3, where its renderer is written)
- [x] `ignore` applies **here and only here**, and the "matches nothing" warning is
      checked against the source tree, never against a bundle
- [x] `sync` write-back — the deferred half of M1. A finding in the detached file
      **refuses** the write-back and says which line: this is the only path by which a
      file enters a bundle after `collect`, so it must not be the hole in §6
- [x] collect must not walk into the active bundle through its own symlinks

✅ Done, and the honest score against the sentence below. `collect` on this machine
writes 176 files and 38 packages; every package carries the line that suggested it.

- ✅ `quickshell`, not the local `noctalia-qs` provide. Two more the same rule caught:
  `matugen` instead of `matugen-bin` and `code` instead of `visual-studio-code-bin` —
  both better than what the hand-written `example/` says.
- ✅ `config/kitty/catppuccin.conf`, which the hand-written manifest forgot.
- ⚠ `ttf-cascadia-mono-nerd` **only with `pacman -Fy`.** There is no files database on
  this machine, so the chain degrades to step 3 exactly as §5.2 says it does: the font
  files ship inside the bundle and a warning names the reason.
- ⚠ `starship` is not found, and correctly so: nothing in the *selected* directories
  names it. It lives in fish's prompt, and `fish/` is not part of the hyprland scan's
  pre-tick. Ticking it would find it.

**A false *negative* turned up later, while M3 was being tested:** `collect kitty` produced
a bundle with an empty `[packages]`. Every suggestion came from a command a config
*launches*, and nothing in `kitty.conf` launches kitty. The selected directory names are
now run through the same chain (`scan/deps.rs::from_selection`), which costs one `which`
per ticked directory and closes the case where the bundle ships a config for a program it
does not install. The default selection barely moves — `hypr` is not a binary — so the M2
score above stands.

Five false positives were found by running it and reading every line, not by reasoning:
`~/.config/zen` (2252 files, pre-ticked because `zen-browser` starts with `zen`),
`texinfo` (from `info=$(bluetoothctl …)`, where the command is what follows `$(`),
`llvm` (from `if not …` in a **python** file read as a shell script), `binutils`
(from `as` inside a quoted jq program), and seven neovim `require "nvchad.options"`
lines reported as dead references (a Lua module is not a path).

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

- [x] `scan/roles.rs` — package → role table (~40 lines), fill in `components`
- [x] `post.rs` — `components` → a shareable list
- [x] `post.rs` — `--format reddit|markdown|plain`
- [x] `post.rs` — copy to clipboard (`wl-copy` / `xclip`, whichever exists)
- [x] The same renderer writes `README.md` during `collect`

**Done means:** the `[components]` block in `docs/standard.md` produces that document's
list exactly.

✅ Done, and *exactly* is the word: the acceptance sentence is a `#[test]` comparing
`render()` against that document's block as one string. It passes.

**The spec was missing its own line-breaking rule**, and writing the renderer is what
found that. The rules table said what a value looks like and in what order roles come, but
not why `**Bar:**` sits alone while `**Terminal:**` and `**Shell:**` share a line. One
rule explains all eleven breaks in the block: **fill greedily to 80 columns.** It is in
`standard.md` now — a spec whose own example cannot be reproduced from it is not a spec.

Three smaller calls, recorded because they are user-visible:

- **`post` takes a path, not just a store name.** `dotpack post example` renders this
  repo's bundle without installing it. The manifest is the only input, so there is nothing
  to activate first, and it makes the command usable on a bundle you are still writing.
- **The generated `README.md` is written once and never regenerated.** It carries the
  markdown list, a `dotpack use` line derived from `homepage`, the package counts, the
  services and a **By hand** section for every `url` component. A file the tool rewrites is
  a file nobody is allowed to improve — and the point of it is to be improved.
- **The role table only fills what the config scan left empty.** `gtk_theme`, `icons`,
  `cursor` and the fonts come from `settings.ini` and `fc-match`; a name read out of the
  config beats a name matched in a package list.

Not done here and not pretending to be: **correcting a role is editing the manifest** until
the TUI (M6). And `roles::fill` runs at collect time only — it does not backfill an
existing bundle, because rewriting someone's hand-written manifest is not a thing a `post`
command should do.

---

## M4 — Sharing · 2 weekends

- [x] Source resolution: `github:U/R[/branch]`, `gitlab:`, `https://`, local paths
- [x] `git clone --depth 1` (real repos are 75 MB+)
- [x] Parse `#variant`, say "not supported in v1"
- [x] Reject a repo without `dotfiles.toml` with a clear message — do not run a foreign `install.sh`
- [x] `requires` version check: strip epoch and pkgrel from `pacman -Q`, compare field by
      field **as integers** (`0.9` vs `0.56`) — warn, do not block
- [x] `wm` mismatch — warn, do not block
- [x] Validation-time reference check on a foreign bundle (`scan/refs.rs`, no machine state)
- [x] Install plan: packages / files / services / hook / manual-step summary + confirmation
- [x] Manual steps: every `components` entry carrying a `url` is printed, never fetched
- [x] Hooks: **show the contents**, confirm, run with `DP_BUNDLE_DIR` / `DP_MODE`
- [x] Hooks run once per bundle — check and update `hooks_ran` in the ledger
- [x] `--yes`, `--no-hooks`, `--run-hooks`, `--as <name>`
- → **moved to M6**: restoring the terminal around package installation. The CLI never
      took it — pacman inherits stdio and streams on its own. It is the alternate screen
      that has to be left and re-entered, and there is no alternate screen until the TUI

**Done means:** `dotpack use github:<your-own-repo>` works on a clean user.

✅ Done. `tests/switch.rs` clones a real repo over `file://` — everything above the
transport is the same code, and a test that needs the network is a test that gets
switched off.

- **The plan is where M4 actually landed**, not the clone. `use <path>` gained twelve
  lines of warning on `example/` the moment the reference check ran on the receiving
  side: the eleven `scripts/quickshell/…` paths `example/README.md` documents as missing,
  plus `$HOME/.zshrc`. The author saw those at collect time; now the receiver sees them
  before anything is placed, which is the half that was missing.
- **`refs::scan` needed a second path per file**, and that is the whole diff for the
  check. A bundle's file sits in the store and its references resolve against where it
  will be *installed*, so the scan takes `(read from, will live at)` pairs;
  `bundle::shipped()` builds them and `scan()` passes the same path twice.
- **The version comparison found its own first case on this machine.** `pacman -Q pacman`
  answers `7.1.0.r9.g54d9411-1` — a `-git` build whose version genuinely cannot be
  compared field by field, which is exactly the "cannot compare" warning
  `manifest.md` describes. It is in the test as the case, not as a hypothetical.
- **`--as` is a store directory name**, so it goes through the manifest's own
  `[a-z0-9._-]+` rule (`manifest::valid_name`, now shared). `--as ../../evil` is a path
  escape wearing a name, and `fetch` renames into that path.
- **A half-finished clone must not become a store entry.** It lands as `.fetching` and is
  only renamed once it has parsed as a bundle; `store_list()` skips dot-names so a clone
  that died mid-way does not turn into a broken row in `ls`.

`file://` was added to the source table for the test, and it is a real git transport —
not a test-only branch in the code.

---

## M5 — `external` Mode · 1 weekend

The way the standard spreads. The reading half can be done at any point after M0; the
writing half is a flag on a command that does not exist until M2.

- [x] `mode = "external"` — do not touch files, install packages, show roles
- [x] The `managed_by` field (informational, the tool is not called)
- [x] `collect --external` — generate only the manifest for an existing chezmoi/stow repo
      **(needs M2)**
- [x] A clear warning in the install summary: "you will place the files with `chezmoi apply`"

**Done means:** a single `dotfiles.toml` added to Brozi's chezmoi repo installs the
packages via `dotpack use` and touches no files.

✅ Done, and it is the cheapest milestone in the plan by a distance — half a weekend, and
most of that was finding the places that *read* files rather than write them.
`tests/switch.rs` covers both halves against a temporary HOME: activating an external
bundle leaves the filesystem bit-for-bit unchanged (the same snapshot comparison M1's
round trip uses), and `collect --external` adds one file to a repo that already has a
README.

- **`links()` was already right; `shipped()` was not.** The write path never had a bug —
  it asks the bundle for links and an external bundle returns none. The receiver-side
  reference check (M4) walks a *second* list, and it would have read chezmoi's
  `dot_config/…` tree and reported every `source ~/…` line in it as dangling. Not ours to
  place means not ours to judge, and that is one guard in the same file as the first.
- **With nothing to link, the plan is the role list.** `post::list` already renders it
  (M3), so external mode prints `Bar: waybar` where a symlink bundle prints its targets.
  No new renderer, and the plan screen stays one function.
- **`managed_by` is read off the repo, never guessed.** The markers each tool leaves at
  the root — `.chezmoiroot` / `.chezmoiignore` / `.chezmoi.toml.tmpl`, `.stow-local-ignore`
  / `.stowrc`, and failing all of those a `dot_*` entry, which is chezmoi's naming and the
  marker in a repo carrying no dot-file at all. Nothing matches → the field is left out and
  a warning says to write it by hand. It is informational, so a wrong value is a wrong line
  in somebody's manifest for nothing.
- **A font that no package provides changes meaning.** In symlink mode it ships inside the
  bundle (§5.2). In external mode nothing ships, so the same finding is a warning saying
  whoever installs this manifest gets it by hand. Found by running it on this machine, where
  `pacman -Fy` has never run and two fonts land in exactly that branch — the message there
  said "is shipped as a file", which in external mode was a sentence about something that
  does not happen.

Run against this machine's real `~/.config` into a directory holding a `dot_config/` and a
README: `managed_by = "chezmoi"`, 38 packages, `dotfiles.toml` the only file added, and the
README untouched.

---

## M6 — TUI · 3 weekends

- [x] Event loop, alternate screen, raw mode
- [x] `std::panic::set_hook` restores the terminal first — **ratatui's own**, installed by
      `ratatui::init()`. Ours would have been the same code with our name on it
- [x] Leave/re-enter the terminal around package installation (moved here from M4)
- [x] Worker thread + `mpsc` — scanning must not freeze the UI
- [x] Main screen: bundle list, active marker, detached + secret counters, detail panel
- [x] Switch plan screen, hook source with `h`
- [x] Collect wizard, 5 steps — warnings screen carries secrets **and** dangling references
- [x] Checklist widget (ratatui has none built in — `List` + our own state)
- [x] Consistent keymap + `?` help
- [x] Terminal palette colors only

**Done means:** every screen in `docs/tui.md` is reachable, and `esc` goes back everywhere
without applying anything.

✅ Done. `dotpack` with no arguments opens it. Nine screens in 1853 lines, tests included,
and the reason it is that small is that every screen calls something M1–M5 already wrote: the plan
screen *is* `apply::plan`, the wizard *is* `scan::collect`, and applying anything prints
through `main.rs`'s own `show` / `report` with the terminal handed back.

- **The panic hook is ratatui's.** `ratatui::init()` installs one that restores the
  terminal before the message prints — §6's non-negotiable, already written by the library.
- **Jobs are the whole "leave the TUI" design (§5).** A key press never installs anything:
  it sets a `Job`, and the loop restores the terminal, runs it, waits for `enter`, and goes
  back in. Packages, `git clone` and hooks all inherit a real terminal, so the sudo prompt
  works and pacman prints its own progress.
- **One worker thread, three messages.** The `~/.config` walk — 2252 files in a single
  directory on this machine — and the scan both run off it, and the event loop's 100 ms
  poll drains the channel for free. A message arriving for a wizard the user has already
  left is dropped rather than applied to the next one.
- **`ls` and the main screen read the same rows.** `bundle::rows()` came out of `main.rs`
  when the TUI needed the same four answers; two implementations of "what is on this
  machine" is how two faces of a tool start disagreeing about it.
- **The plan gained a services section, in both faces.** Switching away from a bundle
  `disable --now`s its units, and until the TUI drew a screen with a SERVICES block nothing
  had ever said so before it happened.
- **`+ add a package by hand` earns its keep on this machine.** `starship` is the case M2
  recorded as a correct miss — nothing in a config launches it. Step 3/5 is where that gets
  fixed without editing the manifest afterwards.

Not done, and named rather than pretended: `/` on the main screen (a list of four bundles
does not need a filter) and `d` on the packages checklist — §7's table gives it to "remove
a package", which is what `space` already does from the other side, and a second key for
one outcome is a keymap that has to be remembered twice.

---

## M7 — Release Prep · 2 weekends

- [x] Test on sway and i3 (not just hyprland)
- [x] Test on a clean user with no helper installed
- [ ] Publish an example bundle repo — the reference people will look at
- [ ] Publish the spec as a document separate from the tool (`docs/standard.md` + `manifest.md`)

### The second machine

A second Arch box — Hyprland and sway installed, **no AUR helper**, no `pacman -Fy`
database, its own `~/.config` — under a throwaway `HOME`, which is the only reason
invariant 14 was worth having. `collect` and a hyprland → sway → hyprland round trip ran
there against the example bundle and a hand-written sway config. Five things came back,
and none of them was visible on the machine the tool was written on.

- **One conflicting name cost all thirty packages.** `pipewire-jack` conflicts with the
  installed `jack2`, pacman refused the transaction, and *nothing* was installed — while
  the switch reported "3 linked" and looked like it had worked. This is the same class as
  "a name no repo has", which `plan` already routes around; the batch now falls back to a
  transaction per package **only when the batch fails**, so a conflict costs one package.
  29 of 30 on the retry.
- **pacman reads a closed stdin as no.** Every transaction ends with
  `Proceed with installation? [Y/n]`, and piped input aborts it — the whole list comes back
  as "not installed" with nothing saying why. It is a fact about the invocation, not a
  cause to guess at afterwards, so it is a plan warning.
- **`set $term foot` hid the terminal from the dependency scan.** Sway's own default
  config names each command once in a `set` line and every binding refers to the variable;
  hyprland's `$terminal` does the same, and the example bundle's
  `bind = $mainMod, T, exec, $terminal` was being missed on this machine all along. The
  table is built over the whole selection first, because the definition and the use are in
  different files.
- **`include ~/.config/sway/config.d/*` was reported dead on every sway bundle.** The last
  line of the config sway ships. A glob resolves to its parent directory now.
- **The example bundle enabled a unit that does not exist.** `easyeffects` ships no user
  unit on Arch — `collect` never fills `services`, so the line was hand-written and wrong,
  and it only failed out loud on a machine that did not already have the rice.

The CLI plan also **names the packages** now instead of counting them. The TUI already
did; "install 30 from repos" is not something a person can approve, and invariant 7 exists
so that they can.

**i3, honestly:** an i3 config was collected there — same `Rules` row as sway, same
extraction, `set $mod`/`bindsym`/`bar { status_command }` all read correctly. i3 itself is
not installed on that box, so `detect()` and `i3-msg reload` were not exercised against a
running i3. Two lines in one table; named rather than claimed.

**No AUR helper is a normal machine, not a broken one.** It gets the repo packages, is told
which names were skipped and why, and is never offered a bootstrap — `install.sh` installs
yay for you, and that is one of the things this tool exists not to do.

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
