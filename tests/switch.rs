//! M1's acceptance test: A → B → `use -` against a temporary HOME, and the filesystem
//! comes back to exactly where it was — adopted backups and created directories
//! included.
//!
//! It drives the real binary rather than the modules, because the property under test is
//! the whole sequence, and a binary crate has nothing to import anyway. Every system
//! command a switch runs is stubbed on PATH: without that, `use example` would enable
//! easyeffects on the machine running the test.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The AUR helpers are in here because `helper()` runs `<helper> --version` to find one,
/// and the real yay creates `~/.cache/yay` and `~/.config/yay` the moment it is invoked —
/// inside the test's HOME, which then never matches the snapshot again.
const STUBBED: &[&str] = &[
    "pacman",
    "sudo",
    "systemctl",
    "fc-cache",
    "hyprctl",
    "paru",
    "yay",
    "pikaur",
    "trizen",
];

#[test]
fn a_to_b_and_back_is_a_round_trip() {
    let env = TestEnv::new("round-trip");
    let a = Path::new(env!("CARGO_MANIFEST_DIR")).join("example");
    let b = env.bundle_b();

    // A user who already has a hypr config and something of their own next to it.
    env.write(".config/hypr/hyprland.conf", "the user's own config\n");
    env.write(".config/mine/notes.txt", "untouched\n");
    let before_anything = env.snapshot();

    env.run(&["use", a.to_str().unwrap(), "-y"]);
    assert_eq!(
        env.read_link(".config/hypr"),
        Some(
            env.home
                .join(".local/share/dotpack/bundles/imperative-hyprland/config/hypr")
        ),
        "the bundle's hypr directory is linked at the depth rule's depth"
    );
    assert!(
        env.backups().any(|p| p.ends_with(".config/hypr")),
        "the user's own config was adopted into the backups, not deleted"
    );
    let on_a = env.snapshot();

    env.run(&["use", b.to_str().unwrap(), "-y"]);
    // Through the store, not at the bundle's own path: a local bundle is a link in the
    // store, so `ls` / `use` / `rm` see a directory like any other (TODO.md Phase 0).
    assert_eq!(
        env.read_link(".config/kitty"),
        Some(
            env.home
                .join(".local/share/dotpack/bundles/b-minimal/config/kitty")
        )
    );
    assert!(env.read_link(".config/hypr").is_none(), "A's link is gone");
    assert_eq!(
        std::fs::read_to_string(env.home.join(".config/hypr/hyprland.conf")).unwrap(),
        "the user's own config\n",
        "and the adopted original came back in its place"
    );
    assert!(
        env.home.join(".local/bin/hello.sh").is_symlink(),
        "local/ links per file"
    );
    assert!(
        env.home
            .join(".local/share/fonts/TestFont/x.ttf")
            .is_symlink(),
        "and creates the directories it needs on the way"
    );

    env.run(&["use", "-", "-y"]);
    assert_same(&on_a, &env.snapshot(), "A → B → back to A");

    // With no previous bundle left to go to, `use -` lands where the machine started.
    env.wipe_previous();
    env.run(&["use", "-", "-y"]);
    assert_same(&before_anything, &env.snapshot(), "and back to nothing");
}

#[test]
fn rm_refuses_while_the_bundle_is_active() {
    let env = TestEnv::new("rm-active");
    let b = env.bundle_b();
    env.run(&["use", b.to_str().unwrap(), "-y"]);

    let out = env.try_run(&["rm", "b-minimal"]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("use -"),
        "it has to say how to get out of the way"
    );
}

// --- harness ---

struct TestEnv {
    root: PathBuf,
    home: PathBuf,
    path: String,
}

impl TestEnv {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("dotpack-test-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("home");
        std::fs::create_dir_all(&home).unwrap();

        // Nothing here is allowed to touch the machine running the test.
        let stubs = root.join("bin");
        std::fs::create_dir_all(&stubs).unwrap();
        for name in STUBBED {
            let script = stubs.join(name);
            // `pacman -T` printing nothing means "everything is satisfied", so no
            // installation is ever attempted.
            std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755))
                .unwrap();
        }
        let path = format!("{}:{}", stubs.display(), std::env::var("PATH").unwrap());
        Self { root, home, path }
    }

    /// A second bundle, deliberately covering what `example/` does not: per-file links
    /// under `home/` and `local/`, and a font directory that has to be created.
    fn bundle_b(&self) -> PathBuf {
        let b = self.root.join("bundle-b");
        for (path, contents) in [
            (
                "dotfiles.toml",
                "name = \"b-minimal\"\nwm = \"hyprland\"\n\n[packages]\npacman = [\"kitty\"]\n",
            ),
            ("config/kitty/kitty.conf", "font_size 12\n"),
            ("home/.testrc", "export B=1\n"),
            ("local/bin/hello.sh", "#!/bin/sh\necho hello\n"),
            ("local/share/fonts/TestFont/x.ttf", "not really a font\n"),
        ] {
            let file = b.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, contents).unwrap();
        }
        b
    }

    fn write(&self, relative: &str, contents: &str) {
        let file = self.home.join(relative);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, contents).unwrap();
    }

    fn try_run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_dotpack"))
            .args(args)
            .env("HOME", &self.home)
            .env("PATH", &self.path)
            .output()
            .unwrap()
    }

    fn run(&self, args: &[&str]) {
        let out = self.try_run(args);
        assert!(
            out.status.success(),
            "dotpack {}\n{}{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn read_link(&self, relative: &str) -> Option<PathBuf> {
        std::fs::read_link(self.home.join(relative)).ok()
    }

    fn backups(&self) -> impl Iterator<Item = PathBuf> {
        walk(&self.home.join(".local/state/dotpack/backups")).into_iter()
    }

    /// Drop `previous` from the ledger, which is the state right after a machine's very
    /// first activation.
    fn wipe_previous(&self) {
        let state = self.home.join(".local/state/dotpack/state.toml");
        let text = std::fs::read_to_string(&state).unwrap();
        let kept: String = text
            .lines()
            .filter(|l| !l.starts_with("previous"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&state, kept).unwrap();
    }

    /// Everything under HOME except the tool's own two directories: the state's
    /// timestamps and backup directory names are new on every run by design, and a
    /// bundle stays in the store after you switch away from it.
    fn snapshot(&self) -> BTreeMap<String, String> {
        let ours = [
            self.home.join(".local/state/dotpack"),
            self.home.join(".local/share/dotpack"),
        ];
        walk(&self.home)
            .into_iter()
            // Their parents count as ours too: `.local/share` exists because the store
            // does, and the store outliving a deactivation is the point of a store.
            .filter(|p| !ours.iter().any(|o| p.starts_with(o) || o.starts_with(p)))
            .map(|path| {
                let name = path.strip_prefix(&self.home).unwrap().display().to_string();
                (name, describe(&path))
            })
            .collect()
    }
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = path.symlink_metadata().map(|m| m.is_dir()).unwrap_or(false);
        found.push(path.clone());
        if is_dir {
            found.extend(walk(&path));
        }
    }
    found.sort();
    found
}

fn describe(path: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let meta = path.symlink_metadata().unwrap();
    let mode = meta.permissions().mode() & 0o7777;
    if meta.file_type().is_symlink() {
        format!("link:{}", std::fs::read_link(path).unwrap().display())
    } else if meta.is_dir() {
        format!("dir:{mode:o}")
    } else {
        format!(
            "file:{mode:o}:{}",
            std::fs::read_to_string(path).unwrap_or_default()
        )
    }
}

fn assert_same(before: &BTreeMap<String, String>, after: &BTreeMap<String, String>, what: &str) {
    let differences: Vec<String> = before
        .keys()
        .chain(after.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|k| before.get(*k) != after.get(*k))
        .map(|k| {
            format!(
                "  {k}\n    before: {:?}\n    after:  {:?}",
                before.get(k),
                after.get(k)
            )
        })
        .collect();
    assert!(
        differences.is_empty(),
        "{what} changed the filesystem:\n{}",
        differences.join("\n")
    );
}
