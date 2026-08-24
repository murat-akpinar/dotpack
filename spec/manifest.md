# dotfiles.toml — Manifest Schema

The only mandatory file at the root of a bundle. Being hand-writable is the primary
design goal: **it holds no file list**, only packages and a few settings.

---

## Minimal valid manifest

```toml
name = "my-hyprland"
wm   = "hyprland"

[packages]
pacman = ["hyprland", "waybar", "kitty"]
yay    = ["matugen-bin"]
```

This much works. Every remaining field has a sensible default.

---

## Full example

```toml
schema      = 1
name        = "shyuuhei-hyprland"
version     = "1.2.0"
description = "A quickshell-based hyprland setup with matugen colors"
author      = "shyuuhei"
homepage    = "https://github.com/shyuuhei/dotfiles"
license     = "MIT"

wm      = "hyprland"
distro  = "arch"
preview = "preview/screenshot.png"
mode    = "symlink"

services = ["hypridle", "swayosd"]

ignore = [
  "config/hypr/settings.json",   # regenerated, and its reader falls back when absent
  "config/hypr/scripts/*.log",
]

[requires]
hyprland = ">=0.56"

[packages]
pacman = ["hyprland", "kitty", "fish", "noto-fonts", "hypridle"]
yay    = ["matugen-bin", "swayosd-git"]   # AUR
paru   = []

[components]
wm       = "hyprland"
bar      = { pkg = "waybar", theme = "forest", from = "adi1090x/waybar-themes" }
terminal = { pkg = "kitty", version = ">=0.48" }
icons    = { pkg = "papirus-icon-theme", name = "Papirus" }

[hooks]
pre_install  = "hooks/pre-install.sh"
post_install = "hooks/post-install.sh"

[[assets]]
src  = "assets/wallpapers"
dest = "~/Pictures/wallpapers"
```

---

## Field Reference

### Identity

| Field | Type | Required | Default | Note |
|---|---|---|---|---|
| `schema` | int | no | `1` | Format version. If the tool sees a version it does not know, it warns and keeps trying. |
| `name` | string | **yes** | — | The directory name in the local store. `[a-z0-9._-]+`. On a collision it is overridden with `--as`. |
| `version` | string | no | `"0.0.0"` | Semver. The tool **does not bump it automatically**, that is the user's job. |
| `description` | string | no | `""` | Shown in the TUI list and in the README. |
| `author` | string | no | — | Display only. |
| `homepage` | string | no | — | Can be opened with the `o` key in the TUI. |
| `license` | string | no | — | Display only. |

### Environment

| Field | Type | Required | Default | Note |
|---|---|---|---|---|
| `wm` | enum | **yes** | — | `hyprland` \| `sway` \| `i3`. Determines the scan rules and the reload command. |
| `distro` | enum | no | `"arch"` | Only `arch` in v1. Another value → "unsupported" warning, the install can still be attempted. |
| `requires` | object | no | `{}` | Minimum versions: `{ "hyprland": ">=0.56" }`. The installed version is compared with `pacman -Q`. **Warns, does not block.** |
| `preview` | string | no | — | Image path relative to the bundle root. Its name is shown in the generated README and in the TUI detail panel. |

`requires` is a real need: the rice that was examined requires "Hyprland ≥ 0.56" and its
installer does a version comparison (real-world.md F7).

A `wm` mismatch **does not block** the install, it warns. Someone may want to install a
sway config on hyprland (to get the files); that is their call.

### packages

```toml
[packages]
pacman = []   # repo packages  → sudo pacman -S --needed
yay    = []   # AUR packages  ─┐
paru   = []   # AUR packages  ─┴→ merged, installed with the receiver's helper
```

- All three fields are optional; a missing one counts as empty.
- `yay` and `paru` go into **the same set**. The only reason they are kept apart is that
  someone writing by hand wants to see the name of the helper they use.
- Helper search order at install time: `paru` → `yay` → `pikaur` → `trizen`. If none is
  present, the TUI asks (install a helper / `makepkg -si` / skip the AUR).
- Repo names are not written. `kitty` can be `cachyos-extra-v3` on one machine and
  `extra` on another — package names are portable, repo names are not.
- If `pacman -S` cannot find the package: it is looked up in the AUR, the user is asked if
  it is found, otherwise it is reported as "not found" and the install continues.
- Reserved, ignored in v1: `flatpak`, `cargo`, `npm`, `pipx`.

### components

A role → component map. **Entirely optional**, does not affect installation, purely
descriptive. The machine-readable form of the "full setup" list people write as prose on
r/unixporn.

Full dictionary, spellings and the `dotpack post` command: [components.md](./components.md)

The short form is the package name (`shell = "zsh"`); the long form carries the `pkg`
`name` `theme` `from` `version` `path` `url` `note` fields.

If a `pkg` inside `components` is missing from the `packages` lists → **a warning, not an
error.** Install logic looks only at `packages`.

**Fonts, themes and cursors are components with a preference order.** `pkg` first — most
hand-installed Nerd Fonts turn out to exist in the repos and `collect` finds them with
`pacman -F` (design.md §5.2). If no package ships it, the files go into
`local/share/fonts/` and the component carries no `pkg` at all. `url` is the last resort
and means one thing only: **"install this yourself"** — it is printed in the summary as a
manual step and is never fetched.

### File behavior

| Field | Type | Default | Note |
|---|---|---|---|
| `mode` | enum | `"symlink"` | `symlink` — the installer places the files. `external` — the installer places **nothing**, another tool does. There is no third value: `copy` was removed (design.md §4.4), and with it the `--copy` / `--symlink` flags. |
| `ignore` | string[] | the list below | Globs relative to the bundle root. **Collect-time only:** a matching path is never written into the bundle. It has no meaning at install time — `~/.config/hypr` is one directory link, there is no per-file decision left to make. A written list is **added to** the default, it does not replace it. |
| `assets` | object[] | `[]` | Destinations outside the convention. `{ "src": "...", "dest": "..." }`. `~` is expanded inside `dest`. **Assets are copied, never linked**, and a switch does not remove them — `dest` is usually a directory the user owns (`~/Pictures/wallpapers`), and adopting it into a bundle would be an unpleasant surprise. An existing file of the same name is not overwritten; it is reported. Copied on **every** activation, which the two rules above make the same thing as once: nothing is removed, so the second activation finds everything already there and writes nothing. A directory `src` is copied file by file, and `dest` mirrors it. |

Always ignored (no need to write these):

```
.git/  node_modules/  *.mp4  *.gif  *.log
Code/  */History/  *Trust Tokens*  *.ovpn
```

`preview/` is **not** on that list, though the media inside it usually is: `*.mp4` and
`*.gif` cover the 21 MB promo video, while the one screenshot the `preview` field points
at is the whole reason the directory exists.

In the two repos that were examined: 21 MB of 76.5 MB was a promo video, 29 MB of 273 MB
was VS Code state. Both are pure dead weight at install time
(real-world.md F8, F16).

**A generated file is only safe to `ignore` if its reader tolerates absence.** That is
the whole test, and it is not a property of the file — it is a property of the line that
reads it. Both of this rice's generated files look identical from the outside and only
one of them can go:

| File | Read by | Safe? |
|---|---|---|
| `settings.json` | `workspaces.sh:40`, with `# fallback to 8` on the line above | ✅ ignore it |
| `colors.conf` | `hyprland.conf:15`, `source =`, unconditional | ❌ ship it |

`source`-ing a file that is not there is a config error on the receiver's screen, and
`ignore` cannot save you from it: `~/.config/hypr` is placed as one directory link, so
there is no install-time decision left to make. Same for machine-specific files —
`ignore`-ing `monitors.conf` does not stop `hyprland.conf` from sourcing it. Ship it with
your values and say "edit this" in the README, or drop the `source` line too.
design.md §7 spells the two options out; the §5.1 reference check reports
the mistake at collect time rather than on the receiver's screen.

There is **no** file list. What goes where comes from the directory layout
([the layout](./README.md)):

```
config/<X>  → ~/.config/<X>
home/<X>    → ~/<X>
local/<X>   → ~/.local/<X>
assets/<X>  → only if declared in the "assets" field
```

### mode = "external"

**`mode = "external"`** — the bundle places no files; `dotfiles.toml` carries only
`packages` + `components`, and chezmoi / stow / bare-git places the files.

```toml
name = "brozi-i3"
wm   = "i3"
mode = "external"
managed_by = "chezmoi"        # informational only; the installer never calls this tool

[packages]
pacman = ["i3-wm", "polybar", "picom", "kitty", "zsh"]
```

In `external` mode an installer **installs the packages, shows the roles, and does not
touch files.** The user runs `chezmoi apply` themselves. With no links to list, the role
list *is* the plan, and the line naming `managed_by` appears twice: once in the plan, once
in the summary after the packages are in. The bundle's own tree is not read either — a
`dot_config/…` path resolves against chezmoi's layout, not ours, so the §5.1 reference
check stays out of it.

A collector writes exactly this file into a repo you already keep and fills `managed_by`
in from that repo's markers — `dotpack collect --external --out <repo>` in the reference
implementation.

This is not a fallback plan, it is **the way the standard spreads.** Adding one file to an
existing chezmoi repo is far lower friction than migrating that repo to our layout. The
format has to make sense without our tool — [components.md](./components.md).

### services

```toml
services = ["hypridle", "swayosd"]
```

`systemctl --user enable --now <unit>` is run. The `.service` suffix is optional. System
services (the ones needing root) are **not supported** — somebody else's bundle should not
enable a root service. If it is required, it goes in the `post_install` hook and the user
sees and approves the hook's contents.

On a switch: services present in the old bundle but not in the new one are
`disable --now`d — **stopping alone is not enough**, an enabled unit comes straight back
on the next login.

### hooks

```toml
[hooks]
pre_install  = "hooks/pre-install.sh"
post_install = "hooks/post-install.sh"
```

- Path relative to the bundle root. A path escaping the bundle (`../`, an absolute path) is **rejected**.
- **The contents are shown before it runs**, and the user approves — the whole script,
  in the install plan, above the confirmation. A script from someone else's repo can do
  damage without root too.
- **They run on the bundle's first activation only.** The ledger remembers; `use A` →
  `use B` → `use A` does not run them a second time. Real hooks append to files
  (real-world.md F4) and appending twice is not undoable. `--run-hooks`
  forces a rerun.
- Can be skipped entirely with `--no-hooks`.
- Working directory is the bundle root. Environment variables: `DP_BUNDLE_DIR`, `DP_MODE`.
- Exit code ≠ 0 → a warning; the install continues and nothing is rolled back. That holds
  for `pre_install` too, which runs before the packages: a hook that could not add a repo
  is not a reason to abandon a switch that has not started.

---

## Validation Rules

Error at load time (installation does not start):

- `name` missing, or does not match `[a-z0-9._-]+`
- `wm` missing, or an unknown value
- `packages` is not an object, or one of the lists is not an array of strings
- a `hooks` path escapes the bundle
- `assets[].dest` is not absolute and does not start with `~`
- the file cannot be parsed

Warning (installation continues):

- `schema` is greater than the known version
- `distro` is not `arch`
- `wm` does not match the machine's WM
- **at collect time only**, an `ignore` glob matches nothing in the source tree (possible
  typo). It is not checked when reading a bundle: an ignored path is by definition absent
  from the bundle, so there every correct glob would match nothing
- a duplicate name in a package list
- a `source` / `include` / `@import` inside a shipped config points at a file the bundle
  does not ship (design.md §5.1) — the single most common way a bundle
  installs and then does not work
- `requires` cannot be compared: `pacman -Q` prints `1:0.56.0-2`, so the epoch and pkgrel
  are stripped and the rest compared **field by field as integers**. `0.9` vs `0.56` is
  the reason a string comparison is not acceptable; a non-numeric field means "cannot
  compare", which is a warning and not a block

---

## Why `dotfiles.toml`, and why TOML

**The name is tool-independent.** `dotpack.toml` could never be a standard; `package.json`
is not `npm.json` either. Another tool must be able to read and apply this file — that is
the condition for being a standard. The name `package.json` would also confuse npm tooling
and editor plugins.

**TOML, not JSON.** The primary design goal is that this file is **hand-writable.** JSON
has no comments and a trailing comma is an error — yet "why is this package here" is
exactly the kind of note that wants a comment. The audience already lives in TOML
(`starship.toml`, `alacritty.toml`). The `toml` crate is as free as `serde_json`.

`state.toml`, written by the tool, uses the same format — one parser, one crate.
