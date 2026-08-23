# The Standard — `components`

This is the project's real claim: **dotfile sharing has no standard, so we are setting
one.**

---

## Observation

People already write a structured list. Every r/unixporn post has it:

```
WM: i3
Bar: polybar themed by adi1090x (forest theme with my modifications)
Compositor: picom (newest version)
Terminal: Kitty 0.48
GTK theme: Catppuccin Blue Dark
Icons: Papirus
Shell: zsh
Prompt: Starship with lightly edited Catppuccin Powerline Mocha preset
Fetch: Fastfetch
Terminal font: JetBrainsMonoNF-Regular
System font: Ubuntu Medium 10
Launcher: Rofi, again by adi1090x (Launchers: type-1 style-6)
File manager: yazi
Editor: Lazyvim
Wallpaper: Here
```

This **already is a manifest.** The problem is not the format but the location: in a
reddit comment, as prose, unreadable by machines. The repo the post links to
([Brozi/dotfiles](https://github.com/Brozi/dotfiles)) contains none of this information —
no package list, no install script.

The standard = moving that list into `dotfiles.toml`.

---

## The `components` field

```toml
[components]
wm          = "i3"
compositor  = "picom"
shell       = "zsh"
fetch       = "fastfetch"
filemanager = "yazi"

bar      = { pkg = "polybar", theme = "forest", from = "adi1090x/polybar-themes", note = "with my own modifications" }
terminal = { pkg = "kitty", version = ">=0.48" }
prompt   = { pkg = "starship", theme = "Catppuccin Powerline Mocha" }
launcher = { pkg = "rofi", from = "adi1090x/rofi", theme = "type-1 style-6" }
editor   = { pkg = "neovim", theme = "LazyVim" }

gtk_theme = { pkg = "catppuccin-gtk-theme-blue", name = "Catppuccin Blue Dark" }
icons     = { pkg = "papirus-icon-theme", name = "Papirus" }

font_terminal = { pkg = "ttf-jetbrains-mono-nerd", name = "JetBrainsMonoNF-Regular" }
font_system   = { pkg = "ttf-ubuntu-font-family",  name = "Ubuntu Medium 10" }

wallpaper  = { path = "config/wallpapers/forest.png" }
calculator = { name = "qalculate-gtk", url = "https://github.com/…" }
```

The long form is written as an **inline table** (`{ ... }`). TOML's `[components.bar]`
sub-table syntax is also valid but traps anyone writing by hand: sub-tables must come
**after** the plain keys of a section. Inline tables remove that ordering problem
entirely.

### Two spellings

**Short** — just the package name: `shell = "zsh"`

**Long** — when there is a theme, a source or a version:

| Field | For what | Example |
|---|---|---|
| `pkg` | Name of the package to install | `"polybar"` |
| `name` | The name as it appears in settings | `"Catppuccin Blue Dark"` |
| `theme` | Variant | `"type-1 style-6"` |
| `from` | Source / attribution | `"adi1090x/polybar-themes"` |
| `version` | Minimum version | `">=0.48"` |
| `path` | A file inside the bundle | `"config/wallpapers/forest.png"` |
| `url` | Something that is not a package | GitHub link |
| `note` | Free text | `"with my own modifications"` |

The `from` field is not decoration: today the attribution in "polybar themed by adi1090x"
has nowhere to go. Rice culture is built on derivation; there should be a field for
saying who you took it from.

A component that has a `url` **is not downloaded.** It is listed as "do this manually" in
the install summary. In the post that was examined, "Calculator: Here", "Search applet: my
own version" and "Autotiling script: Here" are all in this category — downloading and
running a random GitHub script is not something to automate.

---

## Role Dictionary

Fixed roles — the TUI and the generated output show them in a known order:

| Group | Roles |
|---|---|
| Core | `wm` `compositor` `bar` `terminal` `shell` `prompt` |
| Appearance | `gtk_theme` `qt_theme` `icons` `cursor` `colorscheme` `wallpaper` |
| Fonts | `font_terminal` `font_system` |
| Tools | `launcher` `notifications` `lockscreen` `filemanager` `editor` `fetch` `browser` `music` `screenshot` `clipboard` `idle` |

Roles outside the list can be written (`"calculator"`, `"search_applet"`). Unknown roles
are appended alphabetically at the end; they are not an error. The dictionary is advice,
not a constraint.

---

## Relationship With `packages`

There are two fields, each with exactly one authority:

- **`packages`** — *what gets installed.* Install logic looks only here.
- **`components`** — *what it is for.* Descriptive; roles, themes, attributions.

If a `pkg` inside `components` is missing from the `packages` lists → **warning** (not an
error). `collect` fills in both, and someone writing by hand sees the warning and fixes it.

Why they were not merged into one field: most of the packages that get installed have no
role (`libnotify`, `sassc`, `gcc-libs`). Forcing a role onto every package pushes people
into inventing meaningless labels.

---

## `dotpack post`

This command is the reason the standard gets adopted.

```
$ dotpack post my-i3

[i3] forest — catppuccin mocha

**WM:** i3 · **Bar:** polybar (forest, adi1090x — with my own modifications)
**Compositor:** picom · **Terminal:** kitty 0.48
**GTK theme:** Catppuccin Blue Dark · **Icons:** Papirus
**Shell:** zsh · **Prompt:** Starship (Catppuccin Powerline Mocha)
**Launcher:** Rofi (adi1090x, type-1 style-6) · **File manager:** yazi
**Editor:** Neovim (LazyVim) · **Fetch:** fastfetch
**Fonts:** JetBrainsMonoNF-Regular / Ubuntu Medium 10

Dotfiles: https://github.com/user/my-i3
Install: `dotpack use github:user/my-i3`

[copied to clipboard]
```

The same content is also written into `README.md` during `collect`.

The logic is simple: **people already have to write this list by hand.** If the tool
produces it, filling in the format becomes a gain rather than a chore. Adoption comes from
here — not from `import`.

The output format is chosen with `--format reddit|markdown|plain`.

---

## How `collect` Fills In The Roles

A package → role table. Roughly 40 lines, written by hand, does not need to be exhaustive:

```
i3 sway hyprland niri river          → wm
picom hyprland                       → compositor
polybar waybar ags quickshell eww    → bar
kitty alacritty foot wezterm ghostty → terminal
zsh fish bash nushell                → shell
starship oh-my-posh                  → prompt
rofi wofi fuzzel tofi                → launcher
dunst mako swaync                    → notifications
i3lock swaylock hyprlock             → lockscreen
yazi ranger nautilus thunar dolphin  → filemanager
neovim helix emacs micro             → editor
fastfetch neofetch macchina          → fetch
feh swww hyprpaper swaybg            → wallpaper
```

A matching package is assigned its role, the rest stay role-less. The `gtk_theme`,
`icons`, `cursor` and `font_*` roles come from the config scan (`gtk-3.0/settings.ini`,
`fc-match` — [design.md §5](./design.md)). The user corrects them in the TUI.

Nobody dies if the table is incomplete: the role stays empty and the package is still
installed.

---

## Why This Could Become A Standard

| Requirement | Status |
|---|---|
| Corresponds to what people already write | ✅ the r/unixporn list, one to one |
| Hand-writable | ✅ the short form is one line |
| No penalty for refusing to write it | ✅ `components` is entirely optional |
| Immediate return for whoever writes it | ✅ the `post` command, the generated README |
| Does not clash with existing tools | ✅ one file added to the root of a chezmoi/stow repo |

The last item matters: one of the repos that was examined uses chezmoi. `dotfiles.toml`
can be added there too — chezmoi does the file placement, we carry the package list. The
standard does not require changing your file manager.

## The Spec Is Separate From The Tool

If it is to be a standard, it must be implementable without the `dotpack` binary. That
has three requirements:

1. **Versioned.** `schema = 1`. A breaking change comes as `schema = 2`; a reader warns on
   a version it does not know and keeps trying.
2. **Written independently of the reference implementation.** This document +
   [manifest.md](./manifest.md) together are the full specification. Someone else must be
   able to write a reader in Go.
3. **Minimal required fields.** Only `name` and `wm`. Everything else has a default, and
   `components` is entirely optional. A standard that is not cheap to write does not spread.

`mode = "external"` is the most concrete form of this independence: leaving file placement
to another tool and offering only `packages` + `components`. It is in v1
([manifest.md](./manifest.md)), because it is the way to reach the most repos for the
least demand.
