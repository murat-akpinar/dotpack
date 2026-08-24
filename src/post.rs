//! `components` → the list people already write by hand, and the bundle's README.
//!
//! The rules are the spec's, not this file's: [spec/components.md]. What matters about all
//! of them is that the output is a function of the manifest and **nothing else** — no
//! machine state, no network, no `url` ever fetched (invariant 11).

use std::io::Write as _;
use std::process::{Command, Stdio};

use crate::manifest::{Component, Manifest};

#[derive(Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum Format {
    /// Filled lines, ` · ` between roles — one paste into a reddit comment.
    Reddit,
    /// One bullet per role. Also what the generated README carries.
    Markdown,
    /// No markup at all, for anywhere that renders none.
    Plain,
}

// --- dictionary start ---
/// Role → display name, **in the order the output uses**. Roles outside this table are
/// appended alphabetically; they are not an error (components.md, Role Dictionary).
const ROLES: &[(&str, &str)] = &[
    ("wm", "WM"),
    ("compositor", "Compositor"),
    ("bar", "Bar"),
    ("terminal", "Terminal"),
    ("shell", "Shell"),
    ("prompt", "Prompt"),
    ("gtk_theme", "GTK theme"),
    ("qt_theme", "Qt theme"),
    ("icons", "Icons"),
    ("cursor", "Cursor"),
    ("colorscheme", "Colorscheme"),
    ("wallpaper", "Wallpaper"),
    // The two font roles render as one merged line, so the second is skipped where it
    // stands and picked up at the first.
    ("font_terminal", "Fonts"),
    ("font_system", "Fonts"),
    ("launcher", "Launcher"),
    ("notifications", "Notifications"),
    ("lockscreen", "Lockscreen"),
    ("filemanager", "File manager"),
    ("editor", "Editor"),
    ("fetch", "Fetch"),
    ("browser", "Browser"),
    ("music", "Music"),
    ("screenshot", "Screenshot"),
    ("clipboard", "Clipboard"),
    ("idle", "Idle"),
];
// --- dictionary end ---

/// Reddit's line width. Filling greedily to it is the whole line-breaking rule: a long
/// entry pushes the next role down, a short one shares the line.
const WIDTH: usize = 80;

// --- render start ---

/// Header, list, footer — what `dotpack post` prints and copies.
pub fn render(manifest: &Manifest, format: Format) -> String {
    let mut out = String::new();
    let wm = format!("{:?}", manifest.wm).to_lowercase();
    out.push_str(&format!("[{wm}] {}", manifest.name));
    if !manifest.description.is_empty() {
        out.push_str(&format!(" — {}", manifest.description));
    }
    out.push_str("\n\n");
    out.push_str(&list(manifest, format));

    let quote = if format == Format::Plain { "" } else { "`" };
    let mut footer = String::new();
    if let Some(homepage) = &manifest.homepage {
        footer.push_str(&format!("\nDotfiles: {homepage}"));
    }
    if let Some(source) = source(manifest) {
        footer.push_str(&format!("\nInstall: {quote}dotpack use {source}{quote}"));
    }
    if !footer.is_empty() {
        out.push('\n');
        out.push_str(&footer);
    }
    out
}

/// The roles alone, without the header or the install line.
pub fn list(manifest: &Manifest, format: Format) -> String {
    let entries = entries(manifest);
    match format {
        Format::Reddit => pack(
            entries
                .iter()
                .map(|(label, value)| format!("**{label}:** {value}"))
                .collect(),
        ),
        Format::Markdown => entries
            .iter()
            .map(|(label, value)| format!("- **{label}:** {value}"))
            .collect::<Vec<_>>()
            .join("\n"),
        Format::Plain => entries
            .iter()
            .map(|(label, value)| format!("{label}: {value}"))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn entries(manifest: &Manifest) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (role, label) in ROLES {
        match *role {
            "font_system" => continue,
            "font_terminal" => {
                let fonts: Vec<String> = ["font_terminal", "font_system"]
                    .iter()
                    .filter_map(|r| manifest.components.get(*r).map(|c| value(r, c)))
                    .collect();
                if !fonts.is_empty() {
                    out.push((label.to_string(), fonts.join(" / ")));
                }
            }
            _ => {
                if let Some(component) = manifest.components.get(*role) {
                    out.push((label.to_string(), value(role, component)));
                }
            }
        }
    }
    // Unknown roles last. A BTreeMap iterates sorted, which is the alphabetical the spec
    // asks for.
    for (role, component) in &manifest.components {
        if !ROLES.iter().any(|(known, _)| known == role) {
            out.push((title(role), value(role, component)));
        }
    }
    out
}

/// `name` if present, else `pkg`, else `path` — **verbatim**. Package names are lowercase
/// and stay lowercase; `starship` is what you type to install it.
fn value(role: &str, component: &Component) -> String {
    let detail = match component {
        Component::Pkg(pkg) => return pkg.clone(),
        Component::Full(detail) => detail,
    };
    let mut text = detail
        .name
        .as_deref()
        .or(detail.pkg.as_deref())
        .or(detail.path.as_deref())
        .unwrap_or(role)
        .to_string();
    if let Some(version) = &detail.version {
        // Nobody writes `kitty >=0.48` in a post.
        text.push(' ');
        text.push_str(version.trim_start_matches(['>', '<', '=', '^', '~', ' ']));
    }
    let mut inside = [detail.theme.as_deref(), detail.from.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
    if let Some(note) = &detail.note {
        if inside.is_empty() {
            inside = note.clone();
        } else {
            inside.push_str(&format!(" — {note}"));
        }
    }
    if !inside.is_empty() {
        text.push_str(&format!(" ({inside})"));
    }
    if let Some(url) = &detail.url {
        text.push_str(&format!(" — {url}"));
    }
    text
}

fn pack(items: Vec<String>) -> String {
    let mut lines: Vec<String> = Vec::new();
    for item in items {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 3 + item.chars().count() <= WIDTH => {
                line.push_str(" · ");
                line.push_str(&item);
            }
            _ => lines.push(item),
        }
    }
    lines.join("\n")
}

fn title(role: &str) -> String {
    let spaced = role.replace('_', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().to_string() + chars.as_str(),
        None => spaced,
    }
}

/// `homepage` → the argument `use` takes. Only the two forge prefixes it understands;
/// anything else has no install line rather than a wrong one.
fn source(manifest: &Manifest) -> Option<String> {
    let url = manifest.homepage.as_deref()?;
    let (host, path) = ["github", "gitlab"].iter().find_map(|host| {
        url.strip_prefix(&format!("https://{host}.com/"))
            .map(|path| (*host, path))
    })?;
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    // A repo path is `user/repo`; a longer one is a directory inside it, not a source.
    (path.split('/').count() == 2).then(|| format!("{host}:{path}"))
}

// --- render end ---

/// The bundle's own README, written by `collect` (design.md §4.1). It is generated once
/// and never regenerated, so it says so and gets out of the way.
pub fn readme(manifest: &Manifest) -> String {
    let mut out = format!("# {}\n\n", manifest.name);
    if !manifest.description.is_empty() {
        out.push_str(&format!("{}\n\n", manifest.description));
    }
    out.push_str(&list(manifest, Format::Markdown));

    let source = source(manifest).unwrap_or_else(|| format!("path/to/{}", manifest.name));
    out.push_str(&format!(
        "\n\n## Install\n\n```bash\ndotpack use {source}\n```\n"
    ));

    let packages = &manifest.packages;
    let aur = packages.yay.len() + packages.paru.len();
    out.push_str(&format!(
        "\n{} package{} from the repos and {aur} from the AUR — the lists are in \
         `dotfiles.toml`.\n",
        packages.pacman.len(),
        if packages.pacman.len() == 1 { "" } else { "s" },
    ));
    if !manifest.services.is_empty() {
        out.push_str(&format!(
            "\nUser services enabled on install: {}.\n",
            manifest.services.join(", ")
        ));
    }

    // A component carrying a url is a manual step, here as everywhere: nothing is
    // fetched for the reader (invariant 11).
    let manual: Vec<String> = manifest
        .components
        .iter()
        .filter_map(|(role, component)| match component {
            Component::Full(detail) => detail.url.as_ref().map(|url| {
                let what = detail
                    .name
                    .as_deref()
                    .or(detail.pkg.as_deref())
                    .unwrap_or(role);
                format!("- **{}:** {what} — {url}", title(role))
            }),
            Component::Pkg(_) => None,
        })
        .collect();
    if !manual.is_empty() {
        out.push_str(&format!(
            "\n## By hand\n\nNot installed for you, by design:\n\n{}\n",
            manual.join("\n")
        ));
    }

    out.push_str("\n---\n\nWritten by `dotpack collect`. It is never regenerated — edit it.\n");
    out
}

/// `wl-copy`, then `xclip`. Neither present is not an error: the text is on stdout.
pub fn copy(text: &str) -> bool {
    for (binary, args) in [
        ("wl-copy", &[][..]),
        ("xclip", &["-selection", "clipboard"][..]),
    ] {
        // wl-copy on an X session complains at length about a missing wayland socket.
        // That is our fallback working, not an error the user has to read.
        let Ok(mut child) = Command::new(binary)
            .args(args)
            .stdin(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            continue;
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if child.wait().map(|s| s.success()).unwrap_or(false) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `[components]` block of spec/components.md, verbatim.
    const STANDARD: &str = r#"
name        = "forest"
description = "catppuccin mocha"
wm          = "i3"
homepage    = "https://github.com/user/my-i3"

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
"#;

    /// M3's acceptance test: that block renders to that document's list, exactly.
    #[test]
    fn the_standard_renders_to_its_own_example() {
        let manifest: Manifest = toml::from_str(STANDARD).expect("parses");
        let expected = "\
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
Install: `dotpack use github:user/my-i3`";
        assert_eq!(render(&manifest, Format::Reddit), expected);
    }

    #[test]
    fn plain_and_markdown_carry_the_same_values() {
        let manifest: Manifest = toml::from_str(STANDARD).expect("parses");
        assert!(list(&manifest, Format::Plain).starts_with("WM: i3\nCompositor: picom\n"));
        assert!(
            list(&manifest, Format::Markdown)
                .lines()
                .all(|l| l.starts_with("- **"))
        );
        // No markup means no backticks either.
        assert!(
            render(&manifest, Format::Plain).ends_with("Install: dotpack use github:user/my-i3")
        );
    }

    #[test]
    fn the_example_bundles_readme() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example");
        let manifest = Manifest::load(&dir).expect("example loads");
        let readme = readme(&manifest);
        assert!(readme.starts_with("# imperative-hyprland\n"));
        assert!(readme.contains("- **Terminal:** kitty"));
        assert!(readme.contains("73 packages from the repos and 3 from the AUR"));
        // The cursor is the bundle's one manual step, and its url is printed, not fetched.
        assert!(
            readme.contains(
                "- **Cursor:** Bibata-Modern-Ice — https://github.com/ful1e5/Bibata_Cursor"
            )
        );
        // A homepage that is a repo root gives a real install line.
        assert!(readme.contains("dotpack use github:murat-akpinar/dotpack"));
    }
}
