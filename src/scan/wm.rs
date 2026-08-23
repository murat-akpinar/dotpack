//! Which WM is running, and which files and keys carry its dependencies.
//!
//! The tool only *recognizes* a WM so it knows what to read. It never translates one
//! config into another.

use crate::manifest::Wm;
use crate::pkg;

/// What to read, and what carries a command on a line.
pub struct Rules {
    /// The directory under `~/.config` that belongs to this WM.
    pub dir: &'static str,
    /// A line whose first word is one of these carries a command in its value.
    pub exec_keys: &'static [&'static str],
    /// A binding carries its command after this marker: `bind = …, exec, kitty`.
    pub bind_marker: &'static str,
}

pub fn rules(wm: Wm) -> Rules {
    match wm {
        Wm::Hyprland => Rules {
            dir: "hypr",
            exec_keys: &["exec-once", "exec", "exec-shutdown"],
            bind_marker: "exec,",
        },
        Wm::Sway => Rules {
            dir: "sway",
            exec_keys: &["exec", "exec_always", "status_command"],
            bind_marker: " exec ",
        },
        Wm::I3 => Rules {
            dir: "i3",
            exec_keys: &["exec", "exec_always", "status_command"],
            bind_marker: " exec ",
        },
    }
}

/// The session says what it is; when it does not, exactly one of the three being
/// installed is a good enough answer to offer.
pub fn detect() -> Option<Wm> {
    for variable in [
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_DESKTOP",
        "DESKTOP_SESSION",
    ] {
        if let Ok(value) = std::env::var(variable)
            && let Some(wm) = from_name(&value.to_ascii_lowercase())
        {
            return Some(wm);
        }
    }
    let installed: Vec<Wm> = [Wm::Hyprland, Wm::Sway, Wm::I3]
        .into_iter()
        .filter(|wm| pkg::which(binary(*wm)).is_some())
        .collect();
    match installed.as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}

fn from_name(value: &str) -> Option<Wm> {
    if value.contains("hyprland") {
        Some(Wm::Hyprland)
    } else if value.contains("sway") {
        Some(Wm::Sway)
    } else if value.contains("i3") {
        Some(Wm::I3)
    } else {
        None
    }
}

fn binary(wm: Wm) -> &'static str {
    match wm {
        Wm::Hyprland => "Hyprland",
        Wm::Sway => "sway",
        Wm::I3 => "i3",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_names_that_occur_in_the_wild() {
        assert_eq!(from_name("hyprland"), Some(Wm::Hyprland));
        assert_eq!(from_name("sway"), Some(Wm::Sway));
        assert_eq!(from_name("i3"), Some(Wm::I3));
        assert_eq!(from_name("gnome"), None);
    }
}
