# dotpack — Design

Date: 2026-08-23 · Status: design (no code)
Prior research: [research.md](./research.md)

---

## 1. What It Does

In one sentence: **it packages dotfiles together with the packages they need to work,
in a single standard directory format, and installs them with one command.**

The verbs:

| Command | Job |
|---|---|
| `dotpack collect` | Scans the machine's configs + packages, produces a bundle in the standard format |
| `dotpack add <source>` | Downloads a remote/local bundle into the local store (does not install) |
| `dotpack use <name>` | Makes a bundle **active** — this is the rice switch |
| `dotpack ls` | Bundles in the local store, and which one is active |
| `dotpack sync` | Repairs the active bundle — writes a link an application replaced with a real file back into the bundle, then re-links |

`add` + `use` in one step: `dotpack use github:caelestia-dots/shell`

`dotpack` with no arguments → opens the TUI, same jobs from a menu.

Rice switching and the remote source syntax live in a separate document:
[profiles.md](./profiles.md)

**v1 scope:** Arch-based distros, hyprland / sway / i3.
**Out of v1:** other distros, templating, encryption, cross-WM translation, flatpak,
and one-off `copy` installs (§4.4 — `git clone` already does that).

---

## 2. Directory Layout — The Shared Format

The name of this format: **bundle**. One git repo = one bundle.

```
awesome-rice/
├── dotfiles.toml         # manifest — package lists + settings
├── README.md             # human-readable summary, produced by collect
├── config/               → ~/.config/
│   ├── hypr/
│   ├── waybar/
│   ├── fish/
│   └── kitty/
├── home/                 → ~/
│   ├── .bashrc
│   ├── .gitconfig
│   └── .zshrc
├── local/                → ~/.local/
│   ├── bin/
│   └── share/
│       ├── fonts/
│       └── applications/
├── assets/               → destination declared explicitly in dotfiles.toml (wallpapers etc.)
│   └── wallpapers/
└── hooks/                → scripts run at install time (optional)
    ├── pre-install.sh
    └── post-install.sh
```

### Rule: convention > configuration

Where a file lands is implied by **the directory it sits in**. The manifest holds no
file list:

| Path inside the bundle | Destination |
|---|---|
| `config/<X>` | `~/.config/<X>` |  ← depth rule below
| `home/<X>` | `~/<X>` |
| `local/<X>` | `~/.local/<X>` |
| `assets/<X>` | only if a destination is declared in `dotfiles.toml` → `assets` |

Why:
- `dotfiles.toml` stays small, hand-writable (which is what the user actually wants)
- Adding a new file = copying it into a folder. No manifest update needed.
- Someone browsing on GitHub can look under `config/` and see what they are getting
- `assets/` is the exception, because there is no convention for where a wallpaper goes

### Link depth rule

Some rices do not take over the whole config directory, they install **underneath** it —
CyberArch uses `~/.config/hypr/themes/cyberpunk` while the user's own hypr config stays
in place ([real-world.md](./real-world.md) F2). One rule solves both cases:

> Walk down from `config/`. If a directory **contains files directly**, link it and stop.
> If it only contains directories, descend again — repeat until a directory with files
> is reached.

| Bundle content | What gets linked |
|---|---|
| `config/hypr/hyprland.conf` | `~/.config/hypr` |
| `config/hypr/themes/cyberpunk/theme.conf` (no files above it) | `~/.config/hypr/themes/cyberpunk` |

**The depth rule applies to `config/` only.** `home/` and `local/` are linked **per
file**, always. `~`, `~/.local/bin` and `~/.local/share/fonts` are all *mixed*
directories — they hold things that belong to the user, not to any bundle
([real-world.md](./real-world.md) F17: hand-installed Nerd Fonts live exactly there). A
directory link would hide them. Two rules, no exceptions inside either:

| Area | Link granularity |
|---|---|
| `config/` | directory, at the depth-rule depth |
| `home/`, `local/` | per file |

Linking `~/.config/hypr/themes/cyberpunk` means `~/.config/hypr/themes/` may have to be
**created** as a real directory first. Every directory created this way is recorded in
the ledger and removed on deactivation if it is left empty — otherwise switching leaves
litter behind.

The reason `config/` and `home/` are separate: 90% of rices live under `~/.config`, and
`config/` stays visible on GitHub. Dotted files like `home/.bashrc` will be hidden
anyway — unavoidable there.

---

## 3. dotfiles.toml

Full schema: [manifest.md](./manifest.md). Summary:

```toml
name = "shyuuhei-hyprland"
wm   = "hyprland"

services = ["hypridle"]
ignore   = ["config/hypr/scripts/*.log"]   # never collected into the bundle

[packages]
pacman = ["hyprland", "waybar", "kitty", "fish", "noto-fonts"]
yay    = ["matugen-bin", "swayosd-git"]
paru   = []

[hooks]
post_install = "hooks/post-install.sh"
```

### How the helper fields work

The `pacman` / `yay` / `paru` fields mark **where the package is found, not which tool
installs it**:

- the `pacman` list → from official/third-party repos, `sudo pacman -S --needed`
- the `yay` and `paru` lists → **merged and treated as the AUR set**

At install time the tool finds whichever helper is installed on the receiver's machine
(it looks for `paru`, `yay`, `pikaur`, `trizen` in that order) and installs the AUR set
**with that one**. So a bundle that writes a `yay` list gets installed with paru on
someone who uses paru — nobody has to install a second helper. If none is present, the
TUI asks: install a helper / continue with `makepkg -si` / skip the AUR packages.

When `collect` writes the file: AUR packages go into the field of whichever helper is
installed on the machine (this machine has `yay` → the `yay` field). Someone writing by
hand will intuitively use the field of the helper they already use. Both produce the
correct result.

### The repo name problem

On this machine `kitty` comes from the `cachyos-extra-v3` repo. The bundle only says
`"kitty"` — the repo name is not written. Reason: the receiver does not have that repo,
but `kitty` also exists in `extra`. Package names are portable, repo names are not.

If `pacman -S` cannot find it at install time: search the AUR → if found, ask → if not,
report it as "not found" and continue. Installation does not stop over a single package.

---

## 4. Flows

### 4.1 collect

```
1. WM detection        XDG_CURRENT_DESKTOP + installed packages
2. File selection      top-level directories under ~/.config → checklist
                       (the WM-related ones pre-ticked)
3. Reference check     every referenced file ships too, or is reported  (§5.1)
4. Dependency discovery config → command → package → source   (§5)
5. Secret scan         red warnings, unticked by default  (§6)
6. Package confirmation discovered packages as a checklist + manual additions
7. Write               copy into the target directory + generate dotfiles.toml + README.md
8. (optional) git init + first commit
```

The output is a directory. `git remote add` + `push` is the user's job — the tool does
not wrap git.

### 4.2 First activation

```
1. Resolve source      local path or git URL (if a URL, shallow clone into the store)
2. Validate            dotfiles.toml schema, distro match, wm match
                       on mismatch warn but do not stop (the user may still want it)
                       dangling references reported here too  (§5.1)
3. Helper detection    paru > yay > pikaur > trizen; if none, ask
4. Conflict scan       which target files already exist
5. SHOW THE PLAN       "these 34 packages will be installed, these 6 directories backed up and written"
   → nothing happens without the user's confirmation
6. Back up             move into ~/.local/state/dotpack/backups/<timestamp>/
7. pre-install hook    first activation only
8. Install packages    pacman -S --needed  →  <helper> -S --needed
9. Place links
10. fc-cache -f        only if anything landed under ~/.local/share/fonts
11. Services           systemctl --user enable --now <unit>
12. post-install hook  first activation only
13. Summary            installed / skipped / failed, backup path, manual steps (§5.2)
```

**Default decisions** (unless stated otherwise):
- Every conflicting file is **always backed up**, never silently deleted
- The step 5 confirmation is mandatory; it can be skipped with `--yes`
- A single package failing does not stop the install, it is reported at the end
- Hooks are optional and **their contents are shown in the TUI before they run**
  (a script from someone else's repo can do damage without root too)
- **Hooks run on first activation only.** The ledger records that they ran; switching away
  and back does not run them again. Real hooks append to files
  ([real-world.md](./real-world.md) F4) — running them twice duplicates lines. `--run-hooks`
  forces them.
- `fc-cache -f` is not cosmetic: a font that just appeared under `~/.local/share/fonts` is
  invisible to every running application until the cache is rebuilt.

### 4.3 use (rice switching)

Switching between bundles in the local store. Details: [profiles.md](./profiles.md).

```
1. Any detached links in the active bundle → if so, ask (sync / ignore)
2. Install the new bundle's missing packages   (old packages are NEVER removed)
3. Remove the active bundle's symlinks, place the new ones
4. fc-cache -f, if fonts moved
5. Update services (old-but-not-new: `disable --now`)
6. Hooks: only if this bundle has never been activated before
7. Reload the WM  (hyprctl reload / swaymsg reload / i3-msg reload)
```

### 4.4 sync (repairing detached links)

Links are the whole mechanism, so edits made during use are already inside the bundle —
there is nothing to copy back. `sync` exists for the one case where that stops being
true: **an application deleted the link and wrote a regular file in its place.** GTK,
VS Code and anything doing write-to-temp-then-rename do this.

```
sync:
  for every link in the ledger
    target is a link into the active bundle  → fine
    target is a regular file / directory     → DETACHED
      show the diff, offer: write back into the bundle and re-link / leave it / ignore
    target is missing                        → offer to re-link
```

Writing back runs the §6 content scan on what it is about to write. That is the only
moment a file enters the bundle after `collect`, and it must not be the hole in §6.

`ls` and the switch plan report the detached count; the repair itself lives here.

**Why there is no `copy` mode.** A copied install cannot be switched, cannot be synced
and has no ledger — which makes it exactly `git clone` plus `cp -r`. Two flags, a
manifest value, a command and a whole write-back diff engine existed to serve it. They
are gone. `mode` now has two values: `symlink` (default) and `external`
([manifest.md](./manifest.md)); the first is *how dotpack places files*, the second is
*dotpack does not place files*.

---

## 5. Dependency Discovery

The chain (verified on this machine during research):

```
config line → command name → command -v → pacman -Qoq → package → source
```

### Keys scanned per WM

| WM | Files | Keys |
|---|---|---|
| hyprland | `~/.config/hypr/**/*.conf` | `exec-once`, `exec`, what follows `exec,` in `bind*`, `source` (follow it) |
| sway | `~/.config/sway/config`, `config.d/*` | `exec`, `exec_always`, `bindsym … exec`, `status_command`, `include` (follow it) |
| i3 | `~/.config/i3/config` | `exec`, `exec_always`, `bindsym … exec`, `status_command` |

Additionally: commands inside `~/.config/*/scripts/*.sh` (the user's own scripts carry
dependencies too — this machine has 5 scripts under `hypr/scripts/`).

### 5.1 Reference integrity — every referenced file must ship

Config files reference other config files, and **this is not a WM-only habit.** kitty:

```
include ~/.config/kitty/catppuccin.conf
```

A bundle that ships `kitty.conf` without `catppuccin.conf` installs a **broken** kitty —
the colours are gone and kitty prints an error on every start. Following `source` /
`include` only for the WM catches the hypr case and misses this one. So the rule is
general: **every selected text file is scanned for references, and every reference is
resolved.**

| Keyword | Seen in |
|---|---|
| `source` | hyprland, sway, fish, bash |
| `include` | kitty, sway, i3, git, foot |
| `@import` | waybar `style.css`, GTK css, any css |
| `require` / `dofile` | awesome, lua-configured tools |

The keyword table is ~10 lines and does not have to be complete; an unrecognised
directive just means that reference is not checked, exactly as today.

Resolution, and what each outcome means:

| The reference points at | Verdict |
|---|---|
| a file already in the selection | fine, silent |
| a file under `~/.config` that is **not** selected | ⚠ **offer to add it** — the common miss |
| a file under `~` outside `~/.config` | ⚠ offer to add it to `home/` |
| an absolute system path (`/usr/share/…`) | it belongs to a package — feed it to §5's `pacman -Qoq` |
| nothing (the file does not exist) | ⚠ report: the reference is already dead on this machine |

Variables (`$HOME`, `~`) are expanded; anything else containing a `$` is left alone and
reported as "could not resolve" rather than guessed.

The same check runs at **validation time** on somebody else's bundle (§4.2 step 2), where
it is the cheapest possible answer to "will this rice actually work when it lands?" — it
reads the bundle only, no machine state involved.

### Command → package

```
command -v waybar            → /usr/bin/waybar
pacman -Qoq /usr/bin/waybar  → waybar
pacman -Qm | grep waybar     → absent means a repo package, present means AUR
```

`pacman -Qoq` can fail in **two different ways**, and they need different answers:

| | |
|---|---|
| the command is **not installed** | `pacman -F <command>` → the package that would provide it |
| the command is installed but **no package owns it** | `pacman -F <basename>` → the package that *also* ships it |

The second is not a corner case. On this machine:

```
$ command -v starship          → /usr/local/bin/starship
$ pacman -Qoq /usr/local/bin/starship
error: No package owns /usr/local/bin/starship
$ pacman -Ss '^starship$'      → extra/starship 1.26.0-1
```

Installed by the upstream rice's `curl | sh`, while `extra` has shipped it all along.
Without the fallback the component is dropped and the receiver ends up with no prompt.
`/usr/local/bin`, `~/.local/bin` and `~/.cargo/bin` are where this happens.

Only when **both** fail is the command genuinely unpackaged — then it is a script that
belongs in the bundle under `local/bin/`, or a `url` in `components` and a manual step.

`pacman -F` needs the file database (`pacman -Fy`); the TUI says so once, and everything
above degrades to "could not resolve, ask the user" without it.

### Noise removal

Discarded: shell builtins, `systemctl`, the `sh`/`bash`/`env` wrappers, the `uwsm app --`
prefix, everything from coreutils (`sleep`, `cat`, `pkill`…). When a wrapper is seen,
look at **the next token**.

### 5.2 Fonts, themes, icons — three outcomes, in this order

A font is not optional decoration: a rice whose Nerd Font is missing renders every icon
in the bar as a box. So the font chain has to end **somewhere**, and there are exactly
three places it can end.

```
font name in the config  (kitty font_family, gtk settings.ini, waybar css)
   ↓  fc-match "<name>" --format '%{family}\n%{file}'
   ↓  family returned ≠ family asked for  →  the font is NOT installed here → ask the user
   ↓  file
   ├─ 1. pacman -Qoq <file>   → a package owns it            → packages.pacman  ✅
   ├─ 2. pacman -F <basename> → a package SHIPS it, uninstalled → packages.pacman  ✅
   └─ 3. neither              → nothing owns it              → ship the files    ✅
```

**Step 2 is the one that was missing, and it is where most fonts land.** This machine's
terminal font sits in `~/.local/share/fonts/CascadiaMono/`, hand-installed, owned by no
package — [real-world.md](./real-world.md) F17 concluded "no package, ship the file or
declare a url". That was wrong: `extra/ttf-cascadia-mono-nerd` ships that exact font. The
user installed by hand something the repos carry. Searching the file database by
**basename** turns a 40 MB shipped directory into one line in `packages.pacman`.

`pacman -F` needs the file database (`pacman -Fy`); the TUI says so once and the chain
degrades to step 3 without it.

**Step 3 — shipping the files — is a real answer, not a fallback.** The files go into
`local/share/fonts/` (or `local/share/icons/`), they are linked per file like everything
under `local/`, and `apply` runs `fc-cache -f`. The receiver needs no network and no
manual step. *The bundle is the download.*

Same three steps for GTK themes, icon themes and cursors, with
`~/.config/gtk-3.0/settings.ini` → `/usr/share/themes`, `/usr/share/icons`,
`~/.local/share/{themes,icons}` as the search path.

**What is never done: fetching from a `url` at install time.** A `url` in `components`
means *"I could not resolve this to a package and it is not in the bundle"* — it is
printed in the install summary as a manual step and nothing more. Downloading and
unpacking an arbitrary archive from somebody else's manifest is
[real-world.md](./real-world.md) F5 with better manners. If a font matters enough to
share, it belongs in `local/share/fonts/`, and `collect` offers exactly that.

### Accuracy expectations

This discovery **produces suggestions, it does not decide.** False positives and misses
are unavoidable (commands with `$VAR` inside scripts, dynamic calls). The goal: replace
"remember 40 packages from scratch" with "weed 5 lines out of the 45 suggested". Every
line is tickable in the TUI, and packages can be added by hand.

---

## 6. Secret Data — Non-Negotiable

Leaking a token through a shared dotfile is this project's biggest risk. Two layers
during `collect`:

**1. Deny-list (never added by default):**
`.ssh`, `.gnupg`, `.aws`, `.kube`, `.docker/config.json`, `.netrc`,
`.config/gh`, `.config/sops`, `.config/age`, `.config/rclone`,
`.config/mozilla`, `.config/Code`, `.config/discord`, `.local/share/keyrings`,
`*.pem`, `*.key`, `id_rsa*`, `id_ed25519*`,
`*histfile*`, `.bash_history`, `.zsh_history`, `.python_history`, `.node_repl_history`

Shell history was added to the list later: a **public** repo that was examined contained
`zsh/private_dot_histfile` and `gh/private_hosts.yml`
([real-world.md](./real-world.md) F14). chezmoi's `private_` prefix only means
`chmod 600` — it does not prevent publication. The warning text must say
**"this file will be shared"**, not "permissions are restricted".

**2. Content scan** (on the selected files):
`BEGIN * PRIVATE KEY`, `ghp_`, `github_pat_`, `sk-`, `AKIA`, `xoxb-`,
`password\s*=`, `token\s*=`, `api[_-]?key\s*=`

If anything is found: red in the TUI, **unticked by default**, the user has to tick it
deliberately. At the end of `collect` there is also a summary: "possible secret data
found in 3 files, 0 of them added to the bundle."

**The scan cannot stop at `collect`.** In symlink mode `~/.config/fish/config.fish` *is*
the file inside the bundle — editing it writes straight into a git repo that is probably
public. A token added the day after `collect` is seen by nothing. So the content scan
runs at two more points, both of them cheap:

- **`ls` / the main screen** — the active bundle is scanned, and a finding is shown next
  to the detached counter (`active · 2 detached · 1 secret`). Reading a few hundred KB of
  config is not worth optimising.
- **`sync` write-back** — the only path by which a file enters the bundle after
  `collect` (§4.4).

This section does not get simplified.

---

## 7. Portability and Permissions

**Across distros:** package names differ (`i3-wm` on Arch, `i3` on Debian). v1 is
**Arch only**. The manifest says `distro: "arch"`; on another distro the tool warns. A
distro mapping table is a separate, large project.

**Across WMs:** an i3 config is not converted to hyprland, and it is not attempted. The
tool only *recognizes* the WM (so it knows which files to scan), it does not *translate*.

**File permissions:** copying **must** preserve mode bits. A repo that was examined had
56 executable scripts ([real-world.md](./real-world.md) F13); if the exec bit is lost,
the rice breaks silently. Git tracks the exec bit and `std::fs::copy` preserves it — an
implementation that does `read` + `write` by hand does not.

`ponytail:` there is no handling for fine-grained permissions like `chmod 600` / `444`;
git only tracks the exec bit. A `"modes"` field can be added to `dotfiles.toml` if
needed. Let the need be proven first.

**Machine-specific data:** monitor names, DPI and battery presence differ from machine
to machine. In v1 the only remedy is `ignore`, and `ignore` has **exactly one meaning**:

> `ignore` is a **collect-time** filter. A matching path is never written into the bundle.
> It has no meaning at install time.

It cannot have one. `~/.config/hypr` is placed as a *single directory link*; there is no
per-file decision to make on the way in. Whatever is in the bundle is what the user gets.

That has a consequence worth stating plainly, because it is the sharp edge of
[real-world.md](./real-world.md) F15: **if you `ignore` a file, anything that `source`s it
breaks.** `hyprland.conf` with `source = ~/.config/hypr/config/monitors.conf` and no
`monitors.conf` in the bundle is a config error on the receiver's screen. §5.1 catches it
at collect time and says so. The honest options in v1 are both fine, and neither is
silent:

1. **Ship the file** with your values in it, and say in the README that it must be edited.
   Wrong monitor names are a visible, one-line fix.
2. **Ignore the file and remove the `source` line**, letting the tool fall back to its own
   defaults (hyprland auto-detects monitors perfectly well without a `monitors.conf`).

There is no template engine; that is the known ceiling.

---

## 8. Rust Layout

```
dotpack/
├── Cargo.toml
├── docs/
└── src/
    ├── main.rs          # clap: open the TUI when there is no subcommand
    ├── paths.rs         # HOME, store, state, backups — every path starts here
    ├── manifest.rs      # dotfiles.toml serde types, read/write/validate
    ├── bundle.rs        # directory layout rules, path mapping (config/→~/.config)
    ├── pkg.rs           # pacman/expac queries, helper detection, installation
    ├── scan/
    │   ├── mod.rs       # scan orchestration
    │   ├── wm.rs        # WM detection + per-WM key tables
    │   ├── refs.rs      # source/include/@import resolution  (§5.1)
    │   ├── deps.rs      # command → package → source chain
    │   ├── fonts.rs     # fc-match → -Qoq → -F → ship  (§5.2)
    │   ├── roles.rs     # package → role table (fills in components)
    │   └── secrets.rs   # deny-list + content scan
    ├── apply.rs         # THE ONLY WRITER: back up, link, write bundles, services, hooks
    ├── post.rs          # components → shareable list + generated README
    └── tui/
        ├── mod.rs       # event loop, screen routing
        ├── app.rs       # application state (state machine)
        └── screens/     # one file per screen  → tui.md
```

The split is clear: `scan/` only **reads and produces suggestions**, `apply.rs` only
**writes**, `tui/` only **displays and lets the user choose**. No scan function writes to
disk — both testability and the "accidentally break something" risk depend on this.

Two consequences that are easy to get wrong on day one:

- **`collect`'s output is written by `apply.rs` as well** (`write_bundle()`), not by
  `scan/` and not by a `collect.rs`. `collect` is a scan that produces a plan; the plan is
  applied. One writer, no exceptions — otherwise the invariant is false the first time a
  bundle is created.
- **`paths.rs` is the only place `HOME` is read.** M1's acceptance test runs the whole
  A → B → `use -` cycle against a temporary `HOME`; that is impossible if
  `env::var("HOME")` is scattered across eight modules. One function, written before
  anything calls it.

### Dependencies

| Crate | For what |
|---|---|
| `ratatui` + `crossterm` | TUI |
| `serde` + `toml` | `dotfiles.toml` + `state.toml` |
| `clap` (derive) | subcommands |
| `walkdir` | file scanning |
| `anyhow` | error handling |

**Deliberately not added:**
- `git2` → calling the `git` binary with `Command` is enough (cloning is one command)
- `alpm` bindings → parsing `pacman`/`expac` output is enough and less fragile
- `tokio` → nothing is async; package installation is blocking anyway and should stay that way
- `dirs` → `std::env::var("HOME")` is one line, and the tool is Linux-only anyway
- `regex` → config parsing works with `split('=')` + `split_whitespace()`.
  `ponytail:` the secret scan starts with fixed substring matching; if too much escapes,
  `regex` gets added.
- `tui-tree-widget` → file selection is a flat list, not a tree (§ tui.md). A tree can be added if needed.

7 crates. For each one, the answer to "what would I write without it" is 50+ lines.

---

## 9. Open Decisions

They all live in one place now — [TODO.md](../TODO.md) § Phase 0. Three of the five that
used to sit here were already answered in another document, which is exactly the failure
mode a per-file list produces.
