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

wallpaper  = { path = "assets/wallpapers/forest.png" }
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
| `path` | A file inside the bundle | `"assets/wallpapers/forest.png"` |
| `url` | Something that is not a package | GitHub link |
| `note` | Free text | `"with my own modifications"` |

The `from` field is not decoration: today the attribution in "polybar themed by adi1090x"
has nowhere to go. Rice culture is built on derivation; there should be a field for
saying who you took it from.

A component that has a `url` **is not downloaded, by any conforming reader.** That is part
of the spec, not an implementation choice: a manifest is data written by a stranger, and a
format whose fields can make a reader fetch things is a format nobody should run. It is
listed as "do this manually" in the install summary and nothing more. In the post that was
examined, "Calculator: Here", "Search applet: my own version" and "Autotiling script: Here"
are all in this category — downloading and running a random GitHub script is not something
to automate.

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

## Rendering the list — `dotpack post`

The command belongs to the reference implementation; **the rules below are the standard's**,
and any tool that follows them produces the same text. This is the part that gets the
format adopted.

```
$ dotpack post my-i3

[i3] forest — catppuccin mocha

**WM:** i3 · **Compositor:** picom
**Bar:** polybar (forest, adi1090x/polybar-themes — with my own modifications)
**Terminal:** kitty 0.48 · **Shell:** zsh
**Prompt:** starship (Catppuccin Powerline Mocha)
**GTK theme:** Catppuccin Blue Dark · **Icons:** Papirus
**Wallpaper:** assets/wallpapers/forest.png
**Fonts:** JetBrainsMonoNF-Regular / Ubuntu Medium 10
**Launcher:** rofi (type-1 style-6, adi1090x/rofi) · **File manager:** yazi
**Editor:** neovim (LazyVim) · **Fetch:** fastfetch
**Calculator:** qalculate-gtk — https://github.com/…

Dotfiles: https://github.com/user/my-i3
Install: `dotpack use github:user/my-i3`

[copied to clipboard]
```

**The rendering rules, because "it looks about right" is not implementable.** `dotpack
post` has to be a function of the manifest and nothing else:

| Part | Rule |
|---|---|
| Order | the Role Dictionary's order, group by group. Unknown roles last, alphabetically — `calculator` is the last line for exactly that reason |
| Label | the role's display name (`font_terminal` → part of the merged **Fonts** line) |
| Value | `name` if present, otherwise `pkg`, otherwise `path` (that is what makes `wallpaper` a line), **verbatim** — package names are lowercase and stay lowercase. Nothing is title-cased; `starship` is what you type to install it |
| `version` | appended bare, operator stripped: `">=0.48"` → `kitty 0.48` |
| `theme` + `from` | in parens: `theme, from` — then `— note` if there is one |
| `url` | appended after an em dash. Never fetched, here or anywhere |
| Empty roles | omitted entirely; no `**Browser:** —` filler |
| Line breaks | entries are **filled greedily to 80 columns**, ` · ` between them. A long entry pushes the next role onto its own line; that, and nothing else, is why the block above breaks where it does. `**Bar:**` is 78 columns on its own, so `**Terminal:**` starts a new line |

The three formats differ in markup only — same roles, same order, same values:

| `--format` | Shape |
|---|---|
| `reddit` (default) | the block above: filled lines, `**Label:** value` separated by ` · ` |
| `markdown` | one `- **Label:** value` bullet per role. This is what the generated `README.md` carries |
| `plain` | `Label: value`, one per line, no markup — for anywhere that renders none |

The list is copied to the clipboard with `wl-copy`, then `xclip`; neither present is not an
error, the text is on stdout either way.

An earlier version of this block title-cased some values and not others (`Rofi` but
`polybar`), dropped `wallpaper` and `calculator`, and used neither the dictionary order nor
any other. That is fine in a mock-up and fatal in a spec: TODO.md M3 is
judged by whether the `[components]` block above renders to exactly this.

The same renderer writes the bundle's own `README.md` during `collect`: the markdown list,
the `dotpack use` line derived from `homepage`, the package counts, the services that get
enabled, and a **By hand** section listing every component carrying a `url`. It is written
once and never regenerated — a generated file the tool keeps rewriting is a file nobody is
allowed to improve.

The logic is simple: **people already have to write this list by hand.** If the tool
produces it, filling in the format becomes a gain rather than a chore. Adoption comes from
here — not from `import`.

The output format is chosen with `--format reddit|markdown|plain`.

---

## How `collect` Fills In The Roles

A package → role table. Roughly 40 lines, written by hand, does not need to be exhaustive:

```
hyprland sway i3 niri river               → wm
hyprland sway picom                       → compositor
waybar polybar quickshell ags eww         → bar
kitty alacritty foot wezterm ghostty      → terminal
fish zsh nushell bash                     → shell
starship oh-my-posh                       → prompt
rofi wofi fuzzel tofi                     → launcher
dunst mako swaync                         → notifications
hyprlock swaylock i3lock                  → lockscreen
nautilus thunar dolphin yazi ranger       → filemanager
neovim helix emacs micro                  → editor
fastfetch neofetch macchina               → fetch
swww awww hyprpaper swaybg mpvpaper feh   → wallpaper
matugen pywal wallust                     → colorscheme
cliphist clipman copyq                    → clipboard
hypridle swayidle                         → idle
grim grimblast flameshot maim             → screenshot
firefox chromium brave                    → browser
ncmpcpp cmus mpd                          → music
```

**Each row is in preference order**, and that order is the outer loop: a bundle carrying
both `waybar` and `quickshell` gets `bar = "waybar"`. `hyprland` is in two rows on purpose
— on wayland the compositor *is* the WM.

A matching package is assigned its role, the rest stay role-less. The name written is the
**package's**, AUR suffix and all: `colorscheme = "matugen-bin"`, because a `pkg` that
`packages` does not list is the one thing `validate()` warns about, and a generator that
warns about its own output is broken. The suffix comes off for *matching* only, and only
`-git` / `-bin` — a looser rule pulls `zen-browser` into `zen`. The `gtk_theme`,
`icons`, `cursor` and `font_*` roles come from the config scan (`gtk-3.0/settings.ini`,
`fc-match` — design.md §5.2), and **they win**: the table only fills roles
the scan left empty, because a name read out of the config is better evidence than a name
in a package list. The user corrects them in the TUI.

**For these four roles, prefer `pkg` over `url` harder than intuition suggests.** They are
the roles people install by hand, so `pacman -Qoq` reports no owner and the obvious
conclusion is "not a package". It usually is one: ask `pacman -F` with the **file's
basename** before giving up. A hand-unzipped Nerd Font and a `curl | sh` prompt both turn
out to sit in `extra`. Writing `url` where a `pkg` exists costs the reader a manual step
they never needed.

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
