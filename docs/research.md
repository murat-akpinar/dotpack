# dotpack — Pre-Design Research

Date: 2026-08-23 · Status: **superseded where it differs from
[design.md](./design.md)** — kept for the reasoning and the verified commands, not as a
current decision. The clearest example is §3: it concludes the manifest must record the
repo name and its class; `design.md` decided the opposite (package names are portable,
repo names are not) and that decision stands.

## 1. Problem

Sharing dotfiles today means "clone the repo + read the README + install 40 packages by
hand + work out what's missing from the error messages". What breaks is not the files,
it is **the environment the files need**: packages, fonts, themes, script dependencies.

Goal: the user opens `dotpack` → the TUI collects their configs → alongside them it
derives the required package list **together with each package's source** → it produces
a single shareable bundle. The other side installs it with one command.

Scope: i3, sway, hyprland.

---

## 2. Existing Tools (prior art) and the Gap

| Tool | File management | Package list | Source/repo info | Automatic discovery | TUI |
|---|---|---|---|---|---|
| chezmoi | ✅ (templates, encryption) | ⚠️ by hand, only inside `.chezmoiscripts` | ❌ | ❌ | ❌ |
| yadm | ✅ (git-native) | ⚠️ bootstrap script | ❌ | ❌ | ❌ |
| GNU Stow | ✅ (symlink) | ❌ | ❌ | ❌ | ❌ |
| dotbot / dotdrop | ✅ (YAML manifest) | ⚠️ hand-written commands | ❌ | ❌ | ❌ |
| decman | ✅ partial | ✅ (declarative, in Python) | ⚠️ distinguishes AUR | ❌ | ❌ |
| pacdef / decpac | ❌ | ✅ (group files) | ⚠️ per backend | ❌ | ❌ |
| aconfmgr | ✅ /etc focused | ✅ | ✅ | ❌ | ❌ |
| ML4W / JaKooLit / HyDE / end-4 | ✅ but welded to their own rice | ✅ but a fixed list | ❌ | ❌ | ⚠️ bash menu |

**The gap is clear:** file managers know nothing about packages, package managers know
nothing about files, and rice installers are welded to a single rice — they cannot be
used for somebody else's dotfiles. Nobody **reads the config file and derives the
dependencies for you.**

> Honest warning: `chezmoi` + a hand-written `packages.txt` already does 70% of this.
> The only thing that justifies writing a new tool is **automatic dependency discovery**
> (section 4). Without it, this project is a worse copy of chezmoi.

---

## 3. "Can I write yay/paru/pacman next to the package name?" → Yes, but that is the wrong question

Verified on this machine (CachyOS / Arch):

```
$ pacman -Qqen | wc -l      # explicitly installed, from repos   → 233
$ pacman -Qqem              # foreign (AUR / manual) packages    → 6
cliamp  cmd-wrapped  matugen-bin  networkmanager-dmenu-git  swayosd-git  visual-studio-code-bin

$ expac -S '%r' hyprland    → extra
$ expac -S '%r' kitty       → cachyos-extra-v3     # ← third-party repo!
```

**Critical distinction:** `yay`, `paru` and `pikaur` are not a package's *source*, they
are the *tool that installs it*. `matugen-bin` is not a "yay package"; it is an **AUR**
package, and the other side can install it with paru or with `makepkg -si` just as well.

So the field written into the manifest is not `helper: yay`:

```jsonc
{ "name": "matugen-bin", "source": "aur" }
{ "name": "hyprland",    "source": "extra" }
{ "name": "kitty",       "source": "cachyos-extra-v3" }   // missing on the other side → fall back to extra
```

The helper is chosen **by the receiver, at install time** (installed helpers are
detected: on this machine `pacman` + `yay` were found). Without this distinction,
someone who uses paru is forced to install yay — exactly the hassle we are trying to
remove.

**The third-party repo trap:** the `cachyos-extra-v3` example shows that a repo name can
be machine-specific. The manifest must record both the repo name and the class ("AUR,
official, or third-party"); if the receiver does not have the repo, fall back to the
official equivalent or to the AUR.

---

## 4. The Real Value: Deriving Dependencies From Config

The chain works end to end on this machine:

```
config line  →  command  →  pacman -Qoq $(command -v X)  →  package  →  expac -S '%r'  →  source
```

Real output (the user's own hypr config):

| Appears in config | Package | Source |
|---|---|---|
| `kitty` | kitty | cachyos-extra-v3 |
| `fish` | fish | extra |
| `matugen` | matugen-bin | **AUR** |
| `swayosd-server` | swayosd-git | **AUR** |
| `waybar`, `rofi`, `hyprlock` | — | not installed → look up with `pacman -F` |

Signals to scan, per WM:

- **hyprland**: `exec-once =`, `exec =`, `bind = ..., exec, <command>`, `source =` (follow the chain of sub-files — the user's config is split across 8 files)
- **sway/i3**: `exec`, `exec_always`, `bindsym ... exec`, `include`
- **shared**: commands inside `~/.config/*/scripts/*.sh`, `status_command`, `font` lines

The font/theme side works too:
```
$ fc-match "JetBrainsMono Nerd Font"  →  NotoSansMono-Regular.ttf   # ← font MISSING, silent fallback
$ pacman -Qoq <font file>             →  noto-fonts
```
This matters: a missing font does not raise an error, it silently falls back to another
font and the rice just looks "broken". Writing the font into the manifest is one of the
highest-return detectable things.

**Fragile points (honestly):**
- Commands inside shell scripts: a simple regex catches the easy ones, `$VAR` and piped chains escape
- A command that is not installed → needs `pacman -F`, which returns nothing unless `pacman -Fy` has been run
- Wrappers (`uwsm app -- foo`) → the wrapper is caught, not the command
- False positives are unavoidable → **user confirmation in the TUI is mandatory**, not a silent automatic list

Hence the design decision: **discovery produces suggestions, the user ticks the boxes.**
100% accuracy is not the goal; the goal is to replace "remember 40 packages from
scratch" with "weed 5 lines out of the 45 suggested".

---

## 5. Manifest Format (draft — the package.json equivalent)

> **Note (added later):** the draft below was written as JSON, under the name
> `dotfile.json`. The final format is `dotfiles.toml` — the rationale is at the end of
> [manifest.md](./manifest.md). This section is left as-is, as a research record.

Name: `dotfile.json` (or `dottrace.json`). One file, human-readable, diffable.

```jsonc
{
  "schema": 1,
  "name": "shyuuhei-hyprland",
  "wm": "hyprland",
  "distro": "arch",
  "packages": [
    { "name": "hyprland",   "source": "extra",             "reason": "wm" },
    { "name": "kitty",      "source": "cachyos-extra-v3",  "fallback": "extra", "reason": "exec-once" },
    { "name": "matugen-bin","source": "aur",               "reason": "config:matugen" },
    { "name": "noto-fonts", "source": "extra",             "reason": "font" }
  ],
  "files": [
    { "src": "config/hypr",      "dest": "~/.config/hypr",      "mode": "copy" },
    { "src": "config/fish",      "dest": "~/.config/fish",      "mode": "symlink" }
  ],
  "services": ["hypridle"],
  "hooks": { "post_install": "scripts/post.sh" }
}
```

Notes:
- The `reason` field is cheap but very valuable: the receiver can ask "why is this package here?"
- `mode: copy|symlink` — a symlink stays tied to the git repo, a copy is independent. Both are needed.
- `hooks` is a single optional script. Anything more (pre/post hooks for every step) is YAGNI.

---

## 6. Things That Cannot Be Skipped

**Secret data (trust boundary — no laziness here):** `~/.config` can contain SSH keys,
API tokens, `.netrc`, browser profiles, `gh/hosts.yml` (this machine has `.config/gh`).
The collect step requires a fixed deny-list + a simple secret regex scan + a red warning
in the TUI. Leaking a token through a shared dotfile is this project's biggest risk.

**Cross-distro portability:** package names differ (`i3-wm` on Arch, `i3` on Debian).
Proposal: v1 is **Arch only**. The manifest says `distro: "arch"`, and on another distro
the tool says "unsupported". Writing a distro mapping table is a separate, large project.

**Cross-WM portability:** an i3 config cannot be converted to hyprland, and should not be
attempted. The tool only *recognizes* the WM (so it knows which files to scan), it does
not *translate*.

---

## 7. Three Approaches

**A) Manifest + installer (recommended).** The tool produces `dotfile.json`; the files
live in a normal git repo. Install: `dotpack install <repo>`. Compatible with existing
dotfile repos, git handles git, the tool only manages the manifest.
*Plus:* least code, least lock-in. *Minus:* two things (repo + tool) are needed.

**B) Single-file bundle (.tar.zst + manifest).** No repo, one archive is shared.
*Plus:* easiest to share. *Minus:* no update/version tracking, pressure to reinvent git.

**C) chezmoi plugin.** Only do discovery + emit `packages.toml`, let chezmoi handle files.
*Plus:* least code, most mature foundation. *Minus:* forces the user onto chezmoi, narrows the TUI vision.

**Recommendation: A.** It does not reinvent git, it rides on top of existing dotfile
repos, and it leaves the real value (discovery + install) to the tool. Adding B later, as
an `export` command on top of A, is 20 lines.

---

## 8. TUI Framework

| Option | Plus | Minus |
|---|---|---|
| **Go + Bubble Tea** | one static binary (easy to install), proven by lazygit/gh, ready-made widgets (list, checklist, filepicker) | slightly more memory than Rust |
| Rust + ratatui | fastest, single binary, immediate-mode control | more boilerplate, you build the widgets yourself |
| Python + Textual | fastest to prototype | **hard to distribute** — requires a Python environment from the user, which defeats this tool's purpose |

**Recommendation: Go + Bubble Tea.** This tool makes installation easier; its own
installation cannot be hard. A single binary is mandatory, so Python is out. Between Go
and Rust, performance is irrelevant here (a few hundred lines of list rendering), and
Bubble Tea's ready-made checklist/filepicker components cut the required code
significantly.

> **Note (added later): not what was chosen.** The project is **Rust + ratatui**
> ([tui.md](./tui.md), [design.md §8](./design.md)). The single-binary requirement is
> satisfied either way, and the checklist widget Bubble Tea would have donated turns out
> to be one `List` plus our own selection state — a rung on the ladder, not a framework
> choice. This section stays as the research record.

---

## 9. Open Questions (answered — kept as the record)

> **Note (added later):** none of these are open. The one live list is
> [TODO.md](../TODO.md) § Phase 0.

| | Question | Answer |
|---|---|---|
| 1 | symlink or copy by default? | **symlink, and `copy` was removed entirely** ([design.md §4.4](./design.md)) |
| 2 | existing configs during install? | backed up into `~/.local/state/dotpack/backups/`, recorded in the ledger, restored on removal |
| 3 | explicit packages only, or everything discovery finds? | discovery **suggests**, the user ticks ([design.md §5](./design.md)) |
| 4 | package not found on the other side? | search the AUR → ask → report and continue. One package never stops an install |
| 5 | does v1 install, or only collect? | it installs. `use` is the whole point |

---

## Sources

- [dotfiles.github.io — utilities](https://dotfiles.github.io/utilities/)
- [awesome-dotfiles](https://github.com/webpro/awesome-dotfiles)
- [chezmoi](https://www.chezmoi.io/)
- [Best Dotfile Managers 2026](https://briandetering.net/2026/06/25/best-dotfile-managers-2026/)
- [decman — declarative system manager for Arch](https://github.com/kiviktnm/decman)
- [pacdef — multi-backend declarative package manager](https://github.com/steven-omaha/pacdef)
- [decpac](https://github.com/rendaw/decpac)
- [ArchWiki — Arch User Repository](https://wiki.archlinux.org/title/Arch_User_Repository)
- [ArchWiki — pacman/Tips and tricks](https://wiki.archlinux.org/title/Pacman/Tips_and_tricks)
- [Arch Forums — listing AUR vs repo packages](https://bbs.archlinux.org/viewtopic.php?id=272335)
- [Hyprland Wiki — Preconfigured setups](https://wiki.hypr.land/Getting-Started/Preconfigured-setups/)
- [ML4W dotfiles](https://github.com/mylinuxforwork/dotfiles)
- [Bubble Tea vs Ratatui](https://www.glukhov.org/developer-tools/comparisons/tui-frameworks-bubbletea-go-vs-ratatui-rust/)
- [The TUI Renaissance 2026](https://www.youngju.dev/blog/culture/2026-05-14-tui-development-ratatui-bubbletea-ink-textual-terminal-ui-renaissance-deep-dive-2026.en)
