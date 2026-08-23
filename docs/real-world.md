# Real World — Shared Rices That Were Examined

A teardown done so the design rests on what people actually share, not on assumptions.

**Examined 1:** [ARCANGEL0/CyberArch-Dotfiles](https://github.com/ARCANGEL0/CyberArch-Dotfiles)
— the full repo tree + the 986-line `install.sh` were read. *Installer-script type.*

**Examined 2:** [Brozi/dotfiles](https://github.com/Brozi/dotfiles) — an i3 rice managed
with chezmoi, examined locally. *Dotfile-manager type.* This is the repo the r/unixporn
list below links to.

**Could not be reached:** [r/unixporn — "[Hyprland] it was supposed to be just a bar"](https://www.reddit.com/r/unixporn/comments/1vigsds/hyprland_it_was_supposed_to_be_just_a_bar/)
(u/s3rven, 8 August 2026). Reddit returns 403 to both WebFetch and curl; the body and
comments could not be read. But a "full setup" list written in the same style was obtained
(§ Second Case).

---

## The Numbers

| | |
|---|---|
| Repo size | **76.5 MB** |
| File count | **14,140** |
| `install.sh` | **986 lines** |
| `preview/` (mp4 + gif) | 21 MB, 21 files |
| `assets/` | 14,103 entries |
| README | 11.7 KB, **no package list** |

986 lines of bash — that is exactly what we are trying to replace.

---

## Findings

### F1 — Rice repos are not mirrors of `~/.config`

CyberArch's root directory:

```
assets/  components/  scripts/  preview/  node_modules/
core.ts  theme.lua  config.js  env.ts  widget.ts  tsconfig.json
package.json  city.json  install.sh  README.md
```

There is no such thing as `.config`. This is an AGS/Astal project; `install.sh` does the
mapping itself. **Our `config/` `home/` `local/` convention matches no existing rice.**

→ Consequence: [The adoption problem](#the-adoption-problem).

### F2 — A rice installs *underneath* the config directory, it does not take the whole thing

```bash
CANON="$HOME/.config/hypr/themes/cyberpunk"
ln -sfn "$THEME" "$CANON"
```

The user's own `~/.config/hypr/` structure stays; the rice moves in beside it as a
**subdirectory**, and the user's `hyprland.conf` `source`s it.

→ **Design correction.** The rule "link the first-level directory under config/" was
insufficient. The new rule:

> Walk down from `config/`. If a directory **contains files directly**, link it and stop.
> If it only contains directories, descend again until one with files is reached.

- `config/hypr/*.conf` exists → `~/.config/hypr` is linked (old behavior, the 90% case)
- `config/hypr/themes/cyberpunk/*` exists and there are no files under `config/hypr/` →
  `~/.config/hypr/themes/cyberpunk` is linked, the user's hypr config is untouched

One rule, no settings, and both cases come out right — **for `config/`.** It was later
scoped to that one directory: `home/` and `local/` are linked per file, because `~`,
`~/.local/bin` and `~/.local/share/fonts` hold the user's own things and a directory link
would hide them (F17 is exactly that case). See [design.md §2](./design.md).

### F3 — Real installers write into root-owned directories

```bash
sudo tee /etc/pacman.d/hooks/cyberpunk-pkg-notify.hook
sudo tee /etc/pam.d/<...>                    # PAM configuration
sudo install -d -m 755 "$FONTDST"            # system font directory
```

→ Our stance does not change: `dotfiles.toml` **cannot write anything as root**. Those go
into `hooks/post-install.sh`. But it does mean hooks are the **norm**, not the exception.

### F4 — Real installers *append* to existing files

```bash
echo 'export SHELL=/usr/bin/fish' >> "$HOME/.profile"
printf '...' >> "$HOME/.config/fish/config.fish"
```

They do not replace the file, they **append to it**. Neither the symlink nor the copy model
can express this. → A hook's job. No "append" gets added to the manifest format; that is
not YAGNI, it would be outright wrong (impossible to undo).

### F5 — Real installers download binaries from the internet

```bash
wget -q -O "$HOME/.local/bin/cool-retro-term" "$CRT_URL" && chmod +x "$CRT_BIN"
```

Outside the package manager, unsigned, executable. → Showing the hook's contents before
running it is **not a cosmetic feature**, it is the only line of defense.

### F6 — Real installers delete user data

```bash
rm -rf "$HOME/.local/share/omf" 2>/dev/null
```

Silently, without asking. → Proof of why our "nothing is destroyed without a backup"
guarantee is worth something.

### F7 — Rices have a minimum WM version

README: "Hyprland ≥ 0.56". `install.sh` compares the installed version against the current
one (lines 55–56) and upgrades if needed.

→ **`dotfiles.toml` was missing this.** New field: `requires = { hyprland = ">=0.56" }`.
If it does not match, a warning — not a block, the user can still try.

### F8 — Repos are big and carry preview media

21 MB of the 76.5 MB is mp4 and gif inside `preview/`. Those are the r/unixporn post
itself — nobody wants them removed, but at install time they are **completely useless**.

→ Two consequences:
1. `add` must do a **shallow clone**: `git clone --depth 1`
2. Default `ignore` list: `node_modules/`, `.git/`, `*.mp4`, `*.gif`

`preview/` itself is deliberately **not** on that list, though it looks like the obvious
entry: the `preview` field points a bundle at one screenshot inside it
([manifest.md](./manifest.md)). The media extensions remove the 21 MB and leave the
screenshot, which is the part worth keeping.

### F9 — Design decisions that were confirmed

Where CyberArch independently arrived at the same place we did:

| Our design | In CyberArch |
|---|---|
| Helper search order `paru` → `yay` | `command -v paru \|\| command -v yay` (lines 127, 239) |
| Keeping repo and AUR packages in separate sets | the `miss_repo[]` / `miss_aur[]` arrays |
| Back up before overwriting | `cp -f "$CRTCONF" "$CRTCONF.bak.$(date +%s)"` |
| Avoiding needless installs with `--needed` | present in every `pacman -S` call |
| A single package failure does not stop the install | the `\|\| warn "... continuing"` pattern |

The difference in backups: they leave `.bak.<timestamp>` files in place, we collect them in
a single backup directory and record them in the ledger — the only approach that can be
undone.

### F10 — The `pacman -Syu` trap

Line 48 of `install.sh` does a full system upgrade. On Arch this is the riskiest decision a
dotfile installer can make: it is done to avoid partial-upgrade breakage, but it changes
the user's system without their knowledge.

→ **Our decision:** `dotpack` never runs `-Syu`. Only `pacman -S --needed`. If the database
is stale pacman warns anyway, and the user does their own upgrade themselves.

---

## Second Case — Brozi/dotfiles (chezmoi, i3)

A completely different type: no installer script, chezmoi instead.

| | |
|---|---|
| Size | **273 MB** (`dot_config` alone is 118 MB) |
| Files | 774 |
| `dot_config/wallpapers` | 65 MB |
| `dot_config/private_Code` | 29 MB (VS Code state) |
| Install script | **none** |
| Package list | **none** |

### F11 — The package list exists nowhere

No `run_once_*` script, no list in the README, nothing in `.chezmoidata.toml`. The repo is
configs and nothing else. **The only place that says what to install is the r/unixporn
post** — as prose, unreadable by machines, and set to disappear when the post is archived.

This is the clearest proof of why the project exists.

### F12 — People already write a structured list

From the post belonging to that same repo:

```
WM: i3                          Shell: zsh
Bar: polybar (adi1090x forest)  Prompt: Starship (Catppuccin Powerline Mocha)
Compositor: picom               Fetch: Fastfetch
Terminal: Kitty 0.48            Terminal font: JetBrainsMonoNF-Regular
GTK theme: Catppuccin Blue Dark System font: Ubuntu Medium 10
Icons: Papirus                  Launcher: Rofi (adi1090x type-1 style-6)
File manager: yazi              Editor: Lazyvim
```

This is a manifest — just written as prose and in the wrong place. **The standard we want
to set is the machine-readable form of this list.** Details: [standard.md](./standard.md)

Three things in the list do not correspond to packages: "Calculator: Here", "Search applet:
my own version of this project", "Autotiling script: Here" — all GitHub links. Our stance:
these can be declared but **not downloaded**; they are listed as "do this manually" in the
install summary.

### F13 — File permissions are a first-class concern

chezmoi encodes permissions in the file name. In this repo:

| Prefix | Count | Meaning |
|---|---|---|
| `private_` | 123 | `chmod 600` |
| `executable_` | **56** | `chmod +x` |
| `symlink_` | 27 | produce a link, not a file |
| `empty_` | 27 | create an empty file |
| `readonly_` | 11 | `chmod 444` |

There are 56 executable scripts — if the bit is lost, the rice breaks silently. For us this
problem is **solved for free by git** (the exec bit is tracked), but `apply/` **must**
preserve mode bits while copying — `fs::copy` does that, an implementation that calls
`write` by hand does not.

`ponytail:` there is no equivalent for `private_` and `readonly_`. Git only tracks the exec
bit. If needed, `modes = { "config/x/y" = "600" }` gets added to `dotfiles.toml` — YAGNI for
now.

### F14 — `private_` is mistaken for privacy, but it isn't

In this **public** repo, with the `private_` prefix:

```
dot_config/zsh/private_dot_histfile        ← shell history
dot_config/gh/private_hosts.yml            ← GitHub CLI credential file
dot_config/private_Code/…Trust Tokens      ← VS Code state
```

In chezmoi, `private_` only means `chmod 600` — **it does not prevent publication.** The
user marked the file "private" and pushed it to a public repo.

→ To be added to the deny-list: `*histfile*`, `.bash_history`, `.zsh_history`,
`.python_history`, `.node_repl_history`. Shell history is the most likely leak channel for
passwords and tokens typed into a CLI.

→ And the warning text in the TUI has to change: *"this file is mode 600"* is not a
sufficient assurance; the point is that **it is being shared**.

### F15 — Machine-specific data is solved with templates

`.chezmoidata.toml` keeps a separate block per machine:

```toml
[hosts."brozi-laptop"]
dpi = 120           has_battery = true      polybar_height = 45
main_monitor_name = "eDP"                   network_interface_name = "wlp1s0"
[hosts."brozi-desktop"]
dpi = 96            monitor_count = 2       main_monitor_name = "HDMI-1"
```

16 `.tmpl` files consume this data.

→ Our `ignore` field is the crude equivalent: do not put the machine-specific file in the
bundle at all. Monitor names, DPI and battery presence really do differ from machine to
machine.

**Correction, proven in `example/`: `ignore` is usually the wrong answer here, and the
third case shows why.** `monitors.conf` is the textbook machine-specific file, so it went
into `ignore` — and `hyprland.conf` sources it unconditionally, so the receiver gets a
config error instead of a rice. `ignore` is a collect-time filter; it cannot make the
`source` line disappear.

The v1 answer is not `ignore`, it is one of two honest options ([design.md §7](./design.md)):
ship the file with your values and say "edit this" in the README, or drop the `source` line
and let the tool's own defaults apply. `example/` takes the first.

`ponytail:` there is no template engine in v1; that is the known ceiling — and it is a real
ceiling, not a comfortable one. `ignore` can say "leave the file out", which for a sourced
file means "break it". If users ask for templates it comes with `schema = 2`.

### F16 — Bloat categories

65 MB of wallpapers + 29 MB of VS Code state = a third of the 273 MB. The VS Code state is
useless at install time, it was simply added by accident.

→ Added to the default `ignore` list: `Code/`, `*/History/`, `*Trust Tokens*`. Wallpapers
stay — they are part of the rice — but the `collect` screen shows the size (already in the
design, now confirmed).

---

## Third Case — The Machine This Was Designed On

[ilyamiro/imperative-dots](https://github.com/ilyamiro/imperative-dots), customized. The
installer-script type again, and a bigger one: **1898 lines of bash**, with the package
array hardcoded at line 157, a `pacman -Sy`, a `chsh`, root services enabled with sudo,
and a telemetry id written to a version file. The bundle form of this rice lives in
[`example/`](../example/), which is where the counts are kept — one place, so they can
only be wrong once.

### F17 — The dependency chain dead-ends on things no package owns

Running the documented chain over this machine, three of the rice's components returned
nothing from `pacman -Qoq`:

| Component | Where it actually is |
|---|---|
| `starship` | `/usr/local/bin/starship` — installed by its own curl script |
| terminal font | `~/.local/share/fonts/CascadiaMono/…` — hand-installed Nerd Font |
| cursor theme | `~/.local/share/icons/Bibata-Modern-Ice` — hand-installed |

§5 of the design assumed `fc-match` → file → `pacman -Qoq` closes the loop, and
`gtk-3.0/settings.ini` → `/usr/share/icons` → package likewise. Neither holds when the
user installed the thing by hand — which for Nerd Fonts is the common case.

→ Not a new field: `components` already carries `url` for exactly this, and the fonts and
icons belong in `local/share/fonts/` and `local/share/icons/`, shipped by the bundle. But
the scan must **say so** instead of silently dropping the component: "no package owns
this, it will be shipped as a file / declared as a url".

**That conclusion is half wrong — read the correction below before acting on it.** Two of
the three are ordinary packages.

**Correction, found while reviewing this document.** "No package owns it" was taken to
mean "no package exists". For fonts that is usually false:

```
$ fc-match "CaskaydiaMono Nerd Font Mono" --format '%{file}'
/home/…/.local/share/fonts/CascadiaMono/CaskaydiaMonoNerdFontMono-Regular.ttf   # -Qoq: no owner
$ pacman -Ss cascadia
extra/ttf-cascadia-mono-nerd 3.5.1-1 (nerd-fonts)
```

The font was installed by hand from the Nerd Fonts release page; `extra` ships it. `-Qoq`
answers *"which package installed this file"* and correctly says nobody. The question that
actually matters is *"which package **could** provide this file"*, and that is `pacman -F`
against the **basename** — the same file database §5 already uses for commands.

So the font chain has three ends, not two: owned → `-Qoq`; unowned but packaged → `-F`;
genuinely unpackaged → ship the files in `local/share/fonts/` and run `fc-cache -f`
([design.md §5.2](./design.md)). The middle one is where most Nerd Fonts land, and it is
the difference between a bundle carrying 40 MB of `.ttf` and carrying one package name.

### F18 — Preview media, again, in a live rice

23 MB of the 27 MB `scripts/quickshell/` tree is `guide/previews/*.png`. Third repo out of
three carrying screenshots inside the config tree. The default `ignore` list earns its
place.

### F19 — Configs include configs, and not only WM configs

`~/.config/kitty/kitty.conf` on this machine:

```
font_family      family="CaskaydiaMono Nerd Font Mono" style="SemiBold"
include ~/.config/kitty/catppuccin.conf
```

The `example/` bundle in this repo shipped `kitty.conf` and **not** `catppuccin.conf` —
the real directory has four files, the bundle had one. Installing it produces a kitty
that prints an include error on every start and has none of the rice's colours. Nobody
noticed, because nothing checks.

Following `source` / `include` was specified per-WM ([design.md §5](./design.md) key
table), which catches hyprland's eight `source` lines and misses kitty's one `include`.
`@import` in a waybar `style.css` is the same shape of miss.

→ **The check is general, not per-WM**: every shipped text file is scanned, every
reference is resolved, and one that points outside the bundle is reported
([design.md §5.1](./design.md)). It also runs on a *foreign* bundle at validation time,
where it is the cheapest available answer to "will this rice work when it lands?" — no
machine state needed, just the bundle.

**Second correction, from actually running the rule over the whole directory: keywords are
the minority case.** Twelve dangling references in `example/` as it stood, and `include`
accounts for exactly one of them. The other eleven carry no directive at all:

```
exec-once = swayosd-server --style "$HOME/.config/swayosd/style.css"
exec-once = quickshell -p ~/.config/hypr/scripts/quickshell/Shell.qml
SCRIPTS_DIR="$HOME/.config/hypr/scripts/quickshell"
```

A keyword table would have shipped this bundle with a green light. So the extractor is
**any token starting `~/`, `$HOME/` or `$(dirname …)/`**, and the keyword table survives
only for the bare relative case (`include catppuccin.conf`) that has no other marker.

Writing the checker rather than describing it paid for itself twice more: the naive form
reports 25 findings on this bundle, and the 15 false ones are all the same three shapes —
runtime paths (`~/.cache`, `~/.local/state`), the bundle's own README and manifest
describing paths in prose, and `source "$(dirname …)"` where the keyword extractor fires
first and eats the substitution. All three are exclusions in
[design.md §5.1](./design.md) now. Two of the twelve real ones are fixed (the files ship);
ten remain, and `example/README.md` lists them.

### F20 — `-Qoq` can name a package the command is not called, and `-Ss` cannot see it

`example/dotfiles.toml` listed `quickshell-git`. Checking it produced a confident wrong
answer:

```
$ pacman -Ss '^quickshell-git$'          → (nothing)
$ pacman -Qoq $(command -v quickshell)   → noctalia-qs
$ expac -Q '%S' noctalia-qs              → quickshell  quickshell-git
```

`quickshell-git` is not a nonexistent package — it is a **virtual name `noctalia-qs`
provides**, and it is installed on this machine right now. `pacman -Ss` searches names and
descriptions and never provides, so "no such package" was the natural and incorrect
reading. The first pass through this document made exactly that mistake and wrote it down
as a finding.

Two rules, both cheap:

- **Never conclude a package does not exist from `-Ss`.** `pacman -Si <name>` resolves a
  provide.
- **Record the installable name, not the local provider.** `noctalia-qs` is one machine's
  accident; `extra/quickshell` is what the receiver can install. When `-Qoq` returns a
  name that is not the command, the scan has found a `provides` and must offer both.

---

## Comparing The Two Types

| | CyberArch (installer-script) | Brozi (chezmoi) |
|---|---|---|
| Directory layout | a project directory, mapping lives in the script | a `dot_config/` mirror |
| Package list | inside install.sh, scattered | **none at all** |
| Machine-specific data | none | templates + host data |
| Permissions | `chmod` calls | in file name prefixes |
| Install | `./install.sh` (986 lines) | `chezmoi apply` |
| Is it clear what gets installed | partly (if you read the script) | **no, only in the reddit post** |

Both share the same gap: **a machine-readable, portable package list.** One buried it inside
bash, the other never wrote it. `dotfiles.toml` exists for exactly this gap.

---

## The Adoption Problem

A direct consequence of F1: **there are zero repos in the world containing `dotfiles.toml`.**
`dotpack use github:ARCANGEL0/CyberArch-Dotfiles` does not work today.

Three options:

| | What it does | Assessment |
|---|---|---|
| **Reject** | Do not accept a repo outside the format | Honest. The right call for v1. |
| **Wrap** | Find `install.sh` and run it | Worthless — the user can already run it |
| **Import** | Extract the package list from the installer, generate a draft `dotfiles.toml` | A growth lever, but a separate project |

**v1: reject.** If there is no `dotfiles.toml`, a clear error: *"this repo is not in the
dotpack format"*. The first users will be people packaging **their own** rice — `collect`
already exists for them. The sharing network gets built afterwards.

**v2: `dotpack import <repo>`.** Clones the repo, greps the `pacman -S` / `yay -S` lines
inside `install.sh`, tries to map directories into `config/`, and produces a **draft**
`dotfiles.toml`. The user fixes it in the TUI. No promise of an automatically correct
result — reliably parsing 986 lines of bash is not possible. But better than writing from
scratch.

This should be recorded: **the format's value depends on bundles existing in the format.**
`import` is not a technical feature, it is the project's growth mechanism.

---

## What Fed Back Into The Design

- [x] Link depth rule corrected → `design.md` §2, `profiles.md` §3
- [x] `requires` field added → `manifest.md`
- [x] Default `ignore` list added → `manifest.md`
- [x] `preview` field added (for the generated README) → `manifest.md`
- [x] `add` will do a shallow clone → `TODO.md` M4
- [x] `-Syu` will never be run → the invariants in `CLAUDE.md`
- [x] Repo outside the format → clear error; `import` deferred to v2 → `TODO.md`
- [x] Components no package owns → warn, do not drop → `design.md` §5, `TODO.md` M2
- [x] Unowned font → `pacman -F` by basename before shipping files (F17 correction) → `design.md` §5.2
- [x] `fc-cache -f` after fonts land — a font nothing indexed does not exist → `design.md` §4.2
- [x] Reference integrity as a general check, not a per-WM one (F19) → `design.md` §5.1
- [x] References are mostly **paths in argument position**, not keywords (F19) → `design.md` §5.1
- [x] `-Qoq` may return a provider; `-Ss` cannot see provides (F20) → `design.md` §5
