# TUI Design — Rust + ratatui

Goal: a tool that makes installation easier cannot itself be a chore. One binary, screens
understood at a glance, every step reversible.

---

## 1. Screen Map

```
                    ┌──────────────┐
              ┌────▶│  Main screen │◀────┐
              │     │ bundle list  │     │
              │     └──────┬───────┘     │
              │            │             │
        esc   │   ┌────────┼────────┐    │ esc
              │   │        │        │    │
         ┌────┴───▼──┐ ┌───▼────┐ ┌─▼────┴───┐
         │   Add     │ │ Switch │ │   Sync   │
         │ (source)  │ │  plan  │ │   diff   │
         └─────┬─────┘ └───┬────┘ └────┬─────┘
               │           │           │
               └───────────▼───────────┘
                    ┌──────────────┐
                    │    Apply     │  ← leaves the TUI, plain output streams
                    └──────────────┘

    Main screen ──[c]──▶ Collect wizard (5 steps, forward/back)
```

---

## 2. Main Screen

```
┌ dotpack ───────────────────────────────────────────────────────┐
│                                                                │
│   ● my-hyprland      hyprland   34 pkgs   active · 2 changed   │
│   ○ caelestia        hyprland   51 pkgs   github:caelestia…    │
│   ○ minimal-sway     sway       12 pkgs                        │
│                                                                │
│  ────────────────────────────────────────────────────────────  │
│   my-hyprland                                                  │
│   A quickshell-based hyprland setup with matugen colors        │
│   v1.2.0 · MIT · ~/.local/share/dotpack/bundles/…              │
│   config: hypr, fish, kitty, btop, cava   +3                   │
│                                                                │
├────────────────────────────────────────────────────────────────┤
│ ↵ switch  a add  c collect  s sync  d delete  - back  q quit   │
└────────────────────────────────────────────────────────────────┘
```

- `●` is the active bundle. `2 changed` = the number of files whose symlink was broken or
  whose content changed — the one signal that points the user at `sync`.
- The bottom panel is a summary of the selected bundle. There is no separate "detail"
  screen; list + summary is enough.
- `-` → go back to the previous bundle (`use -`).

---

## 3. Switch Plan

Nothing changes without confirmation.

```
┌ Switch: my-hyprland → caelestia ───────────────────────────────┐
│                                                                │
│  PACKAGES                                                      │
│   + 12 to install   ags, quickshell, gnome-bluetooth-3.0, …    │
│   ↻  8 already installed                                       │
│   ⊘  0 to remove         (packages are never removed)          │
│                                                                │
│  FILES                                                         │
│   ↻ ~/.config/hypr        my-hyprland → caelestia              │
│   + ~/.config/ags         new link                             │
│   − ~/.config/cava        link will be removed                 │
│   ⚠ ~/.config/fish        a real file exists → will be backed up│
│                                                                │
│  SERVICES    + caelestia.service   − swayosd.service           │
│                                                                │
│  HOOK        hooks/post-install.sh   [h] show contents         │
│                                                                │
├────────────────────────────────────────────────────────────────┤
│ ↵ apply   h hook   esc cancel                                  │
└────────────────────────────────────────────────────────────────┘
```

`h` → the hook script's full contents in a scrollable window. No running a script from
someone else's repo unseen.

---

## 4. Collect Wizard

Five steps, forward and back with `tab`/`shift-tab`. Each step is its own screen.

### 4.1 File selection

```
┌ Collect · 2/5 · Files ─────────────────────────────────────────┐
│ WM: hyprland (detected)               / search                 │
│                                                                │
│  [x] hypr          12 files   1.2 MB     ← wm                  │
│  [x] fish           4 files    18 KB                           │
│  [x] kitty          2 files     6 KB                           │
│  [ ] Code          89 files   14 MB   ⚠ may contain secrets    │
│  [ ] gh             1 file      1 KB  ⚠ contains a token       │
│  [x] btop           1 file      4 KB                           │
│  [ ] mozilla      2.1k files 180 MB   ⚠ browser profile        │
│                                                                │
│  selected: 6 folders · 34 files · 1.4 MB                       │
├────────────────────────────────────────────────────────────────┤
│ space select  a all  n none  / search  tab next  esc cancel    │
└────────────────────────────────────────────────────────────────┘
```

- A flat list, not a tree. The **top-level folders** under `~/.config` — people already
  think in terms of "hypr, waybar, fish".
- The WM-related ones are pre-ticked.
- ⚠ rows are red and **unticked by default**. They have to be ticked deliberately.

### 4.2 Dependencies

```
┌ Collect · 3/5 · Packages ──────────────────────────────────────┐
│ 47 packages found · 41 ticked            [pacman 38 · AUR 3]   │
│                                                                │
│  [x] hyprland       extra          wm                          │
│  [x] kitty          extra          config: exec-once           │
│  [x] matugen-bin    AUR            config: exec-once           │
│  [x] swayosd-git    AUR            config: exec-once           │
│  [x] noto-fonts     extra          font: gtk-3.0/settings.ini  │
│  [ ] systemd        core           config: exec-once  ⚠ base   │
│  [?] waybar         —              in config, not installed    │
│                                                                │
│  + add a package by hand                                       │
├────────────────────────────────────────────────────────────────┤
│ space select  + add  d delete  tab next  shift-tab back        │
└────────────────────────────────────────────────────────────────┘
```

- The third column says **why** that package is in the list. That is the only way to weed
  out false positives.
- `[?]` = appears in the config but is not installed (found with `pacman -F`).
- `⚠ base` = from the `base`/`base-devel` group, probably unnecessary.

### 4.3 Secret summary (only if something was found)

```
┌ Collect · 4/5 · Warnings ──────────────────────────────────────┐
│                                                                │
│  ⚠ possible secrets in 2 files — none added to the bundle      │
│                                                                │
│   config/gh/hosts.yml         oauth_token: ghp_…               │
│   home/.gitconfig:14          url."https://x:TOKEN@…"          │
│                                                                │
│  To include them anyway, go back to step 2 and tick them.      │
│                                                                │
├────────────────────────────────────────────────────────────────┤
│ tab next   shift-tab back                                      │
└────────────────────────────────────────────────────────────────┘
```

If nothing was found this step is skipped. If something was, it **cannot be skipped** — the
user has to see it.

---

## 5. Apply Time: Leave The TUI

During package installation, git cloning and hook execution **the TUI closes**, plain
terminal output streams, and the TUI reopens when the work is done.

```
[dotpack] installing caelestia…

$ sudo pacman -S --needed ags quickshell …
[sudo] password for shyuuhei:
:: Retrieving packages...
 ags-2.3.0-1  [######################] 100%
…
$ paru -S --needed matugen-bin
…
[dotpack] 12 packages installed, 0 failed
[dotpack] 6 links placed, 1 file backed up
[press enter to continue]
```

Reasons:

1. **The sudo password.** Asking for a password inside a TUI in raw mode either breaks or
   prints the password on screen. The alternative (pre-authorizing with `sudo -v` +
   refreshing on a timer) works but is needlessly complex.
2. **pacman's output is already good.** Redrawing the progress bar, the download speed and
   the conflict prompts in ratatui is hundreds of lines of code for a worse result.
3. **Debuggability.** If the install blows up, the user has the real pacman error in hand,
   not our summary.

`ponytail:` there is no in-TUI progress panel — pacman's own output streams. If an embedded
view is genuinely wanted, `apply.rs` already streams line by line, so adding a panel is easy.

---

## 6. Implementation Skeleton

### Event loop

```
loop {
    terminal.draw(|f| ui::render(f, &app));      // immediate mode: redraw every frame
    if event::poll(100ms)? { app.handle(event::read()?); }
    while let Ok(msg) = rx.try_recv() { app.apply(msg); }   // from the background worker
    if app.should_quit { break; }
}
```

### Long-running work

Scanning (walking `~/.config`, hundreds of `pacman -Qoq` calls) locks the UI. The solution:

- `std::thread::spawn` + `std::sync::mpsc::channel`
- The worker thread sends progress messages (`Msg::Scanned(pkg)`, `Msg::Done(result)`)
- Thanks to the `event::poll` timeout the main loop is already spinning, so the screen
  stays alive

`tokio` is not needed. One worker thread and one channel is enough.

`ponytail:` `pacman -Qoq` spawns a separate process per command — 50 commands = 50 forks.
If the slowness becomes measurable, `pacman -Qlq` output gets read once and mapped in
memory. Measure first.

### Terminal restoration

On a `panic` the terminal stays in raw mode and on the alternate screen — the user's
terminal is left broken. With `std::panic::set_hook` the terminal is restored first, then
the panic message is printed. This **cannot be skipped**, it is not a laziness question.

The same restoration is used by the "leave the TUI" transition in §5 — so the code is
written anyway, at no extra cost.

### Colors

No hardcoded RGB. Terminal palette colors like `Color::Green`, `Color::Red`, `Color::Reset`
are used. It would be ironic for a ricing tool to override the user's own theme.

---

## 7. Keymap

Consistent across all screens:

| Key | Job |
|---|---|
| `j` / `k` / `↓` / `↑` | navigate |
| `g` / `G` | top / bottom |
| `space` | tick / untick |
| `enter` | confirm, advance |
| `esc` | back, cancel |
| `tab` / `shift-tab` | forward / back in the wizard |
| `/` | search (filter the list) |
| `a` / `n` | select all / none |
| `?` | help window |
| `q` / `ctrl-c` | quit |

`enter` is never destructive — it always leads to a plan screen, and the `enter` on the plan
screen applies.

---

## 8. Open Decisions

1. Should `/` search filter the list, or jump to the match?
2. Should a half-finished collect wizard save its state (`--resume`)?
3. Bundle deletion (`d`) confirmation: `y/n`, or type the name?
4. If the terminal is smaller than 80x24: warn and quit, or use a compressed layout?
