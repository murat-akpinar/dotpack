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
| `dotpack sync` | Writes configs changed during use back into the active bundle |
| `dotpack install <source>` | One-off standalone install (`--copy`) — not added to the store, not part of switching |

`add` + `use` in one step: `dotpack use github:caelestia-dots/shell`

`dotpack` with no arguments → opens the TUI, same jobs from a menu.

Rice switching and the remote source syntax live in a separate document:
[profiles.md](./profiles.md)

**v1 scope:** Arch-based distros, hyprland / sway / i3.
**Out of v1:** other distros, templating, encryption, cross-WM translation, flatpak.

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
> If it only contains directories, descend one more level.

| Bundle content | What gets linked |
|---|---|
| `config/hypr/hyprland.conf` | `~/.config/hypr` |
| `config/hypr/themes/cyberpunk/theme.conf` (no files above it) | `~/.config/hypr/themes/cyberpunk` |

The reason `config/` and `home/` are separate: 90% of rices live under `~/.config`, and
`config/` stays visible on GitHub. Dotted files like `home/.bashrc` will be hidden
anyway — unavoidable there.

---

## 3. dotfiles.toml

Full schema: [manifest.md](./manifest.md). Summary:

```toml
name = "shyuuhei-hyprland"
wm   = "hyprland"
mode = "symlink"

services = ["hypridle"]
ignore   = ["config/hypr/config/monitors.conf"]

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
3. Dependency discovery config → command → package → source   (§5)
4. Secret scan         red warnings, unticked by default  (§6)
5. Package confirmation discovered packages as a checklist + manual additions
6. Write               copy into the target directory + generate dotfiles.toml + README.md
7. (optional) git init + first commit
```

The output is a directory. `git remote add` + `push` is the user's job — the tool does
not wrap git.

### 4.2 install / first install

```
1. Resolve source      local path or git URL (if a URL, clone into a temp directory)
2. Validate            dotfiles.toml schema, distro match, wm match
                       on mismatch warn but do not stop (the user may still want it)
3. Helper detection    paru > yay > pikaur > trizen; if none, ask
4. Conflict scan       which target files already exist
5. SHOW THE PLAN       "these 34 packages will be installed, these 6 directories backed up and written"
   → nothing happens without the user's confirmation
6. Back up             move into ~/.dotpack/backups/<timestamp>/
7. pre-install hook
8. Install packages    pacman -S --needed  →  <helper> -S --needed
9. Place files         copy or symlink
10. Services           systemctl --user enable --now <unit>
11. post-install hook
12. Summary            installed / skipped / failed, backup path
```

**Default decisions** (unless stated otherwise):
- Every conflicting file is **always backed up**, never silently deleted
- The step 5 confirmation is mandatory; it can be skipped with `--yes`
- A single package failing does not stop the install, it is reported at the end
- Hooks are optional and **their contents are shown in the TUI before they run**
  (a script from someone else's repo can do damage without root too)

### 4.3 use (rice switching)

Switching between bundles in the local store. Details: [profiles.md](./profiles.md).

```
1. Any unsaved changes in the active bundle → if so, ask (sync / ignore)
2. Install the new bundle's missing packages   (old packages are NEVER removed)
3. Remove the active bundle's symlinks, place the new ones
4. Update services
5. Reload the WM  (hyprctl reload / swaymsg reload / i3-msg reload)
```

### 4.4 sync (writing back)

If installed with `mode: copy`, the bundle falls behind as `~/.config/hypr` is edited.
`sync` shows the diff and copies the selected parts back into the bundle.

With `mode: symlink`, sync is unnecessary — the files already live inside the bundle.

### copy or symlink

| | copy | symlink |
|---|---|---|
| Rice switching | files copied every time, slow, messy | **instant, only the link changes** |
| If the bundle directory moves | unaffected | everything breaks |
| Apps like GTK/VS Code that delete and rewrite the file | fine | the link disappears, silently detached |
| Does `sync` matter | yes | no |

**Decision: bundles added to the local store are symlinked, one-off `--copy` installs
are copies.** The switching feature depends on symlinks — a bundle installed with
`--copy` cannot be changed with `use`, it is a standalone install. The rule in one
sentence: *want switching, use symlinks; want independence, use copies.*

The risk of a symlink being broken is real (third row above). `sync` catches it: if it
finds a regular file where a symlink was expected, it warns and offers to write the
contents back into the bundle.

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

### Command → package

```
command -v waybar            → /usr/bin/waybar
pacman -Qoq /usr/bin/waybar  → waybar
pacman -Qm | grep waybar     → absent means a repo package, present means AUR
```

For a command that is not installed, `pacman -F <command>` (the file database; the TUI
warns if `pacman -Fy` is needed).

### Noise removal

Discarded: shell builtins, `systemctl`, the `sh`/`bash`/`env` wrappers, the `uwsm app --`
prefix, everything from coreutils (`sleep`, `cat`, `pkill`…). When a wrapper is seen,
look at **the next token**.

### Font / theme / icons

- `font` lines in the config → `fc-match "<name>"` → the returned file → `pacman -Qoq`
- `fc-match` **silently falls back to another font if it cannot find the requested one**
  (on this machine, asking for "JetBrainsMono Nerd Font" returned Noto). If the returned
  family name does not match what was asked for → "font missing" warning; it cannot be
  captured as a package, so the user is asked.
- `~/.config/gtk-3.0/settings.ini` → theme/icon/cursor names → the matching directory
  under `/usr/share/themes`, `/usr/share/icons` → `pacman -Qoq`

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
to machine. In v1 the only remedy is `ignore` — leaving the file out of the bundle
entirely. There is no template engine; that is the known ceiling
([real-world.md](./real-world.md) F15).

---

## 8. Rust Layout

```
dotpack/
├── Cargo.toml
├── docs/
└── src/
    ├── main.rs          # clap: open the TUI when there is no subcommand
    ├── manifest.rs      # dotfiles.toml serde types, read/write/validate
    ├── bundle.rs        # directory layout rules, path mapping (config/→~/.config)
    ├── pkg.rs           # pacman/expac queries, helper detection, installation
    ├── scan/
    │   ├── mod.rs       # scan orchestration
    │   ├── wm.rs        # WM detection + per-WM key tables
    │   ├── deps.rs      # command → package → source chain
    │   ├── fonts.rs     # fc-match + gtk theme detection
    │   ├── roles.rs     # package → role table (fills in components)
    │   └── secrets.rs   # deny-list + content scan
    ├── apply.rs         # back up, copy/symlink, enable services, run hooks
    ├── post.rs          # components → shareable list + generated README
    └── tui/
        ├── mod.rs       # event loop, screen routing
        ├── app.rs       # application state (state machine)
        └── screens/     # one file per screen  → tui.md
```

The split is clear: `scan/` only **reads and produces suggestions**, `apply.rs` only
**writes**, `tui/` only **displays and lets the user choose**. No scan function writes to
disk — both testability and the "accidentally break something" risk depend on this.

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

No code starts before these are answered:

1. **Bundle name/identity:** must `name` in `dotfiles.toml` be unique, or is it just a
   display name? (If there is any future registry/index idea, it should be considered now.)
2. **Where `collect` writes:** is `~/dotfiles/` the default, or is `--out` mandatory?
3. **Versioning:** is the `version` field bumped by hand, or does `sync` bump it automatically?
4. **Does `install` support git URLs in v1**, or does the user clone and pass a local path?
5. **README.md generation:** should collect produce it, or leave a template for the user to write?
