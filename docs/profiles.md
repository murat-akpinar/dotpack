# Profiles — Local Store, Rice Switching, Remote Sources

Inspiration: `nix run github:caelestia-dots/shell#with-cli` — running something from a
remote source in one line. We want the same comfort for dotfiles.

---

## 1. Local Store

Bundles live in one place:

```
~/.local/share/dotpack/
└── bundles/
    ├── my-hyprland/        ← your own bundle (can be a git repo)
    ├── caelestia/          ← added from github
    └── minimal-sway/

~/.local/state/dotpack/
├── state.toml             ← which bundle is active + which links were placed
├── previous               ← the previously active bundle (for use -)
└── backups/
    └── 2026-08-23T14-02-11/  ← the original files taken over during the first install
```

Bundle directories are ordinary git repos. You can `cd
~/.local/share/dotpack/bundles/my-hyprland` and `git push` — the tool does not wrap git,
it only holds the directory.

---

## 2. Source Syntax

```
dotpack use github:caelestia-dots/shell
dotpack use github:user/repo/dev          # branch
dotpack add gitlab:user/repo
dotpack add https://git.example.com/x.git
dotpack add ./local/folder
dotpack add ~/GIT/my-dotfiles
```

Resolution:

| Prefix | Expands to |
|---|---|
| `github:U/R` | `https://github.com/U/R.git` |
| `github:U/R/BRANCH` | the same, with `--branch BRANCH` |
| `gitlab:U/R` | `https://gitlab.com/U/R.git` |
| `https://…`, `git@…` | used directly |
| `./`, `~/`, `/` | local folder — not copied, used **where it is** |

Not copying a local path matters: while working on your own dotfile repo, you want the
file you edit to be written into your repo, not into a second copy.

The bundle name comes from the `name` field in `dotfiles.toml`; on a collision it can be
renamed with `--as <name>`.

### `#variant` — reserved for now

The `github:user/repo#catppuccin` syntax is parsed but returns an "unsupported" error in
v1. In-bundle theme variants will be added later through a `variants` field in
`dotfiles.toml`. Reserving the syntax now avoids a breaking change later.

---

## 3. How Switching Works

The whole switch is **redirecting symlinks**. No files are copied, so it is instant.

### Link rules

| Inside the bundle | Destination | Link type |
|---|---|---|
| `config/hypr/` | `~/.config/hypr` | **directory** link |
| `config/hypr/themes/x/` (no files above it) | `~/.config/hypr/themes/x` | **directory** link, deep |
| `home/.bashrc` | `~/.bashrc` | file link |
| `local/bin/foo.sh` | `~/.local/bin/foo.sh` | file link |
| `local/share/fonts/CascadiaMono/*.ttf` | `~/.local/share/fonts/CascadiaMono/*.ttf` | file link, one per font |

Two rules, and neither has an exception:

- **`config/` → directory link**, at the depth the
  [design.md §2 depth rule](./design.md) picks. That is exactly the unit a rice switch
  wants to swap: `~/.config/hypr` belongs entirely to bundle A or entirely to B.
- **`home/` and `local/` → per file.** `~`, `~/.local/bin` and `~/.local/share/fonts` are
  *mixed* directories: they hold things that are the user's, not any bundle's. A directory
  link there hides them — and `~/.local/share/fonts` is precisely where hand-installed
  Nerd Fonts live ([real-world.md](./real-world.md) F17), so this is not hypothetical.

Any intermediate directory that has to be **created** to place a link
(`~/.config/hypr/themes/` for the deep case, `~/.local/share/fonts/CascadiaMono/`) is
recorded in the ledger and deleted on deactivation **if it is empty**. Without that, every
switch leaves a little more litter behind.

After any link lands under `~/.local/share/fonts`, `apply` runs `fc-cache -f`. A font
nothing has indexed is a font that does not exist as far as every running application is
concerned.

### The link ledger (`state.toml`)

The one requirement for a clean switch: **knowing what you put there.**

```toml
active       = "my-hyprland"
activated_at = "2026-08-23T14:02:11Z"
services     = ["hypridle"]
hooks_ran    = ["my-hyprland", "caelestia"]   # bundles whose hooks have run once

[[links]]
target = "~/.config/hypr"
kind   = "dir"

[[links]]
target          = "~/.config/fish"
kind            = "dir"
adopted_backup  = "2026-08-23T14-02-11/fish"

[[links]]
target = "~/.bashrc"
kind   = "file"

[[links]]
target   = "~/.local/share/fonts/CascadiaMono/CaskaydiaMonoNerdFontMono-Regular.ttf"
kind     = "file"
mkdirs   = ["~/.local/share/fonts/CascadiaMono"]   # created by us, removed if left empty
```

`adopted_backup`: there was **a real file not belonging to the tool** at that path, and
it was moved into the backups. When all bundles are removed it is restored from there.
The user's own config is never lost.

`mkdirs`: directories that did not exist and had to be created to place this link. On
deactivation they are removed **only if empty** — the user may have put their own files
in there since.

`hooks_ran`: hooks are a first-activation thing ([manifest.md](./manifest.md)). This list
is the memory that makes `use A` → `use B` → `use A` safe; without it, every hook that
appends to a file ([real-world.md](./real-world.md) F4) corrupts a little more on each
round trip.

### The `use B` algorithm

```
old = state.toml.links
new = the links B would produce

1. Is the active bundle detached?
   - a regular file where a symlink was expected → an application overwrote the link
   - ask in the TUI: write back to the bundle (sync) / ignore / cancel

2. Does B ship everything it references? (design.md §5.1)
   - a dangling source/include → warn in the plan, do not block

3. Packages: install the ones in B's list that are not installed
   → the old bundle's packages are NOT removed

4. Apply the link diff:
   - only in old  → remove the link, restore adopted_backup if there is one,
                    remove any mkdirs left empty
   - in both      → repoint the link at the new bundle
   - only in new  → place the link (if a real file is in the way, back it up first)

5. fc-cache -f, if anything under ~/.local/share/fonts changed

6. Services: `disable --now` the ones in old but not in new, enable the new ones

7. Hooks: only if B is not in state.toml's hooks_ran

8. Update state.toml + previous

9. Reload the WM:  hyprctl reload | swaymsg reload | i3-msg reload
   (if the WM differs, no reload — warn "log out of the session" instead)
```

`dotpack use -` → returns to the bundle recorded in `previous`. Same idea as `cd -`;
trying a new rice and going back with one command when you don't like it will be this
tool's most-used feature.

### Why packages are not removed

Bundle A wants waybar, bundle B wants ags. After switching to B, waybar stays. Reason:
removing packages is slow, can break the dependency chain, and is hard to undo. A few
hundred MB of disk is cheap compared to a broken system.

`ponytail:` `use --prune` (suggest packages no bundle wants anymore) can be added later;
the removal is still done by `pacman`, the tool only presents the list.

---

## 4. Exactly One Active Bundle At A Time

Partial activation (waybar from A, hypr from B) is not supported. Reason: rices are
designed as a whole — the waybar config depends on hypr keybinds, and the color file
depends on both. Mixing them is the source of "it doesn't work" reports.

Someone who wants to mix two rices creates a third bundle. That is not a feature the tool
needs to support; it is something `collect` already does.

---

## 5. How It Looks In The TUI

The main screen is the bundle list itself:

```
┌ dotpack ──────────────────────────────────────────────────────┐
│                                                               │
│   ● my-hyprland      hyprland   34 packages   active          │
│   ○ caelestia        hyprland   51 packages   github:caeles…  │
│   ○ minimal-sway     sway       12 packages                   │
│                                                               │
│                                                               │
├───────────────────────────────────────────────────────────────┤
│ ↵ switch  a add  c collect  s sync  d delete  - back  q quit  │
└───────────────────────────────────────────────────────────────┘
```

`enter` → the switch plan is shown → confirm → switch. Detailed screens: [tui.md](./tui.md)

---

## 6. Open Decisions

Moved to [TODO.md](../TODO.md) § Phase 0, together with every other open question. Four
documents each keeping their own list is how three of `design.md`'s five questions came to
be answered somewhere else without anyone noticing.
