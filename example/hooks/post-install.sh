#!/bin/sh
# The two things a manifest is not allowed to do: enable a root service, and change your
# login shell. dotpack prints this file in full and waits for you to approve it before it
# runs — which is the only reason a bundle may do them at all.
#
# Runs once, on this bundle's first activation, after the links are in place.

# --- generated configs start ---
# Five files under config/ are not hand-written — this rice generates them from
# templates/, and what ships here is the *author's* machine: their monitors, their
# /home/<name> in env.conf. Upstream's install.sh runs this line; nothing else does, and
# without it the receiver inherits somebody else's display layout and home directory.
#
# It rewrites files inside the bundle, which is what symlink mode means: ~/.config/hypr
# *is* the bundle. dotpack has no templating of its own (design.md §7) and does not need
# any here — the rice brought its own.
#
# The compiler takes the wallpaper directory out of settings.json, which is runtime state
# and not shipped — so on a fresh install it reads `{}` and writes `WALLPAPER_DIR=` with
# nothing after it. Seeded here, the UI has a directory to put images in; what goes in it
# is still yours (the bundle ships no wallpapers, see the README).
#
# Set every time rather than only when empty. settings.json lives at
# ~/.config/hypr/settings.json, which is *inside* the bundle, so a value written on one
# machine is still sitting there on the next one — and "is it already set" cannot tell
# this machine's path from somebody else's. A hook runs once per bundle per machine, so
# once is exactly when this should happen; afterwards the settings UI owns the file.
settings="$HOME/.config/hypr/settings.json"
# `xdg-user-dir PICTURES` SUCCEEDS with $HOME when the machine has no user-dirs.dirs, so
# a `|| fallback` never fires — the answer has to be looked at, not just tested.
pictures=$(xdg-user-dir PICTURES 2>/dev/null)
if [ -z "$pictures" ] || [ "$pictures" = "$HOME" ]; then
    pictures="$HOME/Pictures"
fi
mkdir -p "$pictures/Wallpapers" "$(dirname "$settings")"
# Through jq, so the settings UI's other keys survive; a missing or unparseable file
# falls back to writing the one key.
if ! jq --arg d "$pictures/Wallpapers" '.wallpaperDir = $d' "$settings" > "$settings.new" 2>/dev/null; then
    printf '{"wallpaperDir":"%s/Wallpapers"}\n' "$pictures" > "$settings.new"
fi
mv "$settings.new" "$settings"

~/.config/hypr/scripts/settings_watcher.sh --compile
# --- generated configs end ---

# --- root services start ---
# `services` in dotfiles.toml is user units only; these are root's. A unit that is not
# installed is skipped rather than attempted: a package that failed to install must not
# take the rest of this file with it.
#
# power-profiles-daemon is the odd one out — nothing among the shipped files reads it,
# because the thing that does is the quickshell UI this bundle does not ship (README).
for unit in NetworkManager bluetooth power-profiles-daemon swayosd-libinput-backend; do
    if systemctl cat "$unit.service" >/dev/null 2>&1; then
        sudo systemctl enable --now "$unit.service"
    else
        echo "skipped $unit.service — no such unit on this machine"
    fi
done
# --- root services end ---

# --- login shell start ---
# components.shell = "fish". Upstream's install.sh chshes to zsh without asking; this
# asks for your password, and does nothing at all if the shell is already right.
fish=$(command -v fish)
current=$(getent passwd "$(id -un)" | cut -d: -f7)
# Both sides resolved: /bin is a symlink to /usr/bin on Arch, so /bin/fish and
# /usr/bin/fish are the same shell spelled two ways.
if [ -n "$fish" ] && [ "$(readlink -f "$current")" != "$(readlink -f "$fish")" ]; then
    chsh -s "$fish"
fi
# --- login shell end ---

# The display manager is deliberately left alone. Upstream enables sddm; sddm is not in
# this bundle's [packages], and enabling one display manager disables the one you use.
