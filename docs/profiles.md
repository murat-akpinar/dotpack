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
| `local/share/fonts/` | `~/.local/share/fonts` | directory link |

Which depth gets linked is decided by the
[design.md §2 link depth rule](./design.md): the first directory that contains files.

Under `config/` a **directory** link is used, because that is exactly the unit a rice
switch wants to swap: `~/.config/hypr` belongs entirely to bundle A or entirely to B.
`~/.local/bin` on the other hand is a mixed directory (it holds scripts that do not
belong to the tool), so links there are placed per file.

### The link ledger (`state.toml`)

The one requirement for a clean switch: **knowing what you put there.**

```toml
active       = "my-hyprland"
activated_at = "2026-08-23T14:02:11Z"
services     = ["hypridle"]

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
```

`adopted_backup`: there was **a real file not belonging to the tool** at that path, and
it was moved into the backups. When all bundles are removed it is restored from there.
The user's own config is never lost.

### The `use B` algorithm

```
old = state.toml.links
new = the links B would produce

1. Is the active bundle dirty?
   - a regular file where a symlink was expected → an application overwrote the link
   - ask in the TUI: write back to the bundle (sync) / ignore / cancel

2. Packages: install the ones in B's list that are not installed
   → the old bundle's packages are NOT removed

3. Apply the link diff:
   - only in old  → remove the link, restore adopted_backup if there is one
   - in both      → repoint the link at the new bundle
   - only in new  → place the link (if a real file is in the way, back it up first)

4. Services: stop the ones in old but not in new, start the new ones

5. Update state.toml + previous

6. Reload the WM:  hyprctl reload | swaymsg reload | i3-msg reload
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
│ enter switch  a add  c collect  s sync  d delete  q quit      │
└───────────────────────────────────────────────────────────────┘
```

`enter` → the switch plan is shown → confirm → switch. Detailed screens: [tui.md](./tui.md)

---

## 6. Open Decisions

1. Should local-path bundles (`add ~/GIT/my-dotfiles`) be held in the store as a
   **symlink**, or as an absolute path in `state.toml`? (a symlink is more visible)
2. If package installation fails during `use`: finish the switch or roll it back?
3. What happens when deleting a bundle (`d`) that is active — is `use -` required first?
4. Switching while the active bundle's git repo has uncommitted changes: warn or block?
