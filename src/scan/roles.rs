//! package → role. Fills in `components`, which install logic never reads: it is the
//! r/unixporn list, moved into the manifest (docs/standard.md).
//!
//! A hand-written table, deliberately not exhaustive. Nobody dies if a role stays empty —
//! the package is installed either way.

use std::collections::BTreeMap;

use crate::manifest::{Component, Packages};

// --- table start ---
/// Role → the packages that fill it, best first. `hyprland` is in two rows on purpose:
/// on wayland the compositor *is* the WM.
const TABLE: &[(&str, &[&str])] = &[
    ("wm", &["hyprland", "sway", "i3", "niri", "river"]),
    ("compositor", &["hyprland", "sway", "picom"]),
    ("bar", &["waybar", "polybar", "quickshell", "ags", "eww"]),
    (
        "terminal",
        &["kitty", "alacritty", "foot", "wezterm", "ghostty"],
    ),
    ("shell", &["fish", "zsh", "nushell", "bash"]),
    ("prompt", &["starship", "oh-my-posh"]),
    ("launcher", &["rofi", "wofi", "fuzzel", "tofi"]),
    ("notifications", &["dunst", "mako", "swaync"]),
    ("lockscreen", &["hyprlock", "swaylock", "i3lock"]),
    (
        "filemanager",
        &["nautilus", "thunar", "dolphin", "yazi", "ranger"],
    ),
    ("editor", &["neovim", "helix", "emacs", "micro"]),
    ("fetch", &["fastfetch", "neofetch", "macchina"]),
    (
        "wallpaper",
        &["swww", "awww", "hyprpaper", "swaybg", "mpvpaper", "feh"],
    ),
    ("colorscheme", &["matugen", "pywal", "wallust"]),
    ("clipboard", &["cliphist", "clipman", "copyq"]),
    ("idle", &["hypridle", "swayidle"]),
    ("screenshot", &["grim", "grimblast", "flameshot", "maim"]),
    ("browser", &["firefox", "chromium", "brave"]),
    ("music", &["ncmpcpp", "cmus", "mpd"]),
];
// --- table end ---

/// Roles already in `components` are left alone: the font and theme scans got there from
/// the config, which is better evidence than a package name.
pub fn fill(components: &mut BTreeMap<String, Component>, packages: &Packages) {
    let installed: Vec<&String> = packages
        .pacman
        .iter()
        .chain(&packages.yay)
        .chain(&packages.paru)
        .collect();

    for (role, candidates) in TABLE {
        if components.contains_key(*role) {
            continue;
        }
        // The table's order is the preference order, so it is the outer loop: waybar
        // beats quickshell when a bundle has both.
        if let Some(found) = candidates
            .iter()
            .find_map(|name| installed.iter().find(|p| base(p) == *name))
        {
            components.insert(role.to_string(), Component::Pkg((*found).clone()));
        }
    }
}

/// `swayosd-git` is the AUR name of `swayosd`. Only those two suffixes come off — a
/// looser match pulls `zen-browser` into `zen`.
fn base(package: &str) -> &str {
    package.trim_end_matches("-git").trim_end_matches("-bin")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_bundle_roles() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example");
        let manifest = crate::manifest::Manifest::load(&dir).expect("example loads");
        let mut components = BTreeMap::new();
        fill(&mut components, &manifest.packages);

        // The AUR name is what gets written: components.pkg has to name something
        // `packages` actually lists, or validate() warns about its own output.
        assert_eq!(components["colorscheme"].pkg(), Some("matugen-bin"));
        assert_eq!(components["bar"].pkg(), Some("quickshell"));
        assert_eq!(components["shell"].pkg(), Some("fish"));
        assert_eq!(components["compositor"].pkg(), Some("hyprland"));
        assert_eq!(components["idle"].pkg(), Some("hypridle"));
        // Nothing in the bundle is a launcher package — the role stays empty rather than
        // being guessed at.
        assert!(!components.contains_key("launcher"));
        assert_eq!(manifest.validate().unwrap(), Vec::<String>::new());
    }

    #[test]
    fn a_filled_role_is_not_overwritten() {
        let mut components = BTreeMap::new();
        components.insert("bar".to_string(), Component::Pkg("eww".to_string()));
        fill(
            &mut components,
            &Packages {
                pacman: vec!["waybar".into()],
                ..Default::default()
            },
        );
        assert_eq!(components["bar"].pkg(), Some("eww"));
    }
}
