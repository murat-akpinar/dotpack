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

    /// A bundle that arrives the way a shared one does: over git, with hooks. The hooks
    /// append to a file under HOME, which is exactly what real ones do and exactly why
    /// running them twice has to be impossible (real-world.md F4).
    fn git_repo(&self) -> PathBuf {
        let repo = self.root.join("shared-rice");
        for (path, contents) in [
            (
                "dotfiles.toml",
                "name = \"shared-rice\"\nwm = \"hyprland\"\n\n[hooks]\npre_install  = \"hooks/pre.sh\"\npost_install = \"hooks/post.sh\"\n",
            ),
            ("config/shared/shared.conf", "shared = 1\n"),
            (
                "hooks/pre.sh",
                "#!/bin/sh\necho \"pre $DP_MODE $DP_BUNDLE_DIR\" >> \"$HOME/hook.log\"\n",
            ),
            (
                "hooks/post.sh",
                "#!/bin/sh\necho \"post $DP_MODE $DP_BUNDLE_DIR\" >> \"$HOME/hook.log\"\n",
            ),
        ] {
            let file = repo.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, contents).unwrap();
            if path.starts_with("hooks/") {
                std::fs::set_permissions(
                    &file,
                    std::os::unix::fs::PermissionsExt::from_mode(0o755),
                )
                .unwrap();
            }
        }
        git(&repo, &["init", "-q"]);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-qm", "first"]);
        repo
    }

    fn store(&self, name: &str) -> PathBuf {
        self.home.join(".local/share/dotpack/bundles").join(name)
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

/// The one case links do not cover: an application deleted our link and wrote a real file
/// in its place. `sync` puts that version into the bundle and links again — unless doing
/// so would put a secret in a repo that is probably public.
#[test]
fn sync_writes_a_detached_file_back() {
    let env = TestEnv::new("sync");
    let b = env.bundle_b();
    env.run(&["use", b.to_str().unwrap(), "-y"]);

    // What GTK and VS Code do: unlink, then write.
    let target = env.home.join(".testrc");
    std::fs::remove_file(&target).unwrap();
    std::fs::write(&target, "export B=2\n").unwrap();

    let out = env.try_run(&["ls"]);
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("1 detached"),
        "ls has to show it: {}",
        String::from_utf8_lossy(&out.stdout)
    );

    env.run(&["sync"]);
    assert_eq!(
        std::fs::read_to_string(b.join("home/.testrc")).unwrap(),
        "export B=2\n",
        "the application's version is now the bundle's"
    );
    assert!(target.is_symlink(), "and the link is back");

    // Now the same thing, with a token in it.
    std::fs::remove_file(&target).unwrap();
    std::fs::write(&target, "export GH_TOKEN=ghp_abcdef1234567890\n").unwrap();
    let out = env.try_run(&["sync"]);
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(said.contains("NOT written back"), "{said}");
    assert_eq!(
        std::fs::read_to_string(b.join("home/.testrc")).unwrap(),
        "export B=2\n",
        "the bundle is unchanged"
    );
}

/// M4: a bundle that arrives as a git repo. The clone, the hook the receiver had to
/// approve, and the one rule about hooks that cannot be undone if it is wrong — they run
/// on the **first** activation and never again, because real hooks append to files.
///
/// The remote is a `file://` repo rather than github: everything above the transport is
/// the same code, and a test that needs the network is a test that gets disabled.
#[test]
fn a_cloned_bundle_runs_its_hooks_exactly_once() {
    let env = TestEnv::new("share");
    let url = format!("file://{}", env.git_repo().display());
    let log = env.home.join("hook.log");

    env.run(&["use", &url, "-y"]);
    assert!(
        env.read_link(".config/shared")
            .is_some_and(|t| t.ends_with("shared-rice/config/shared")),
        "the clone is in the store and its config is linked out of it"
    );
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        format!(
            "pre symlink {}\npost symlink {}\n",
            env.store("shared-rice").display(),
            env.store("shared-rice").display()
        ),
        "both hooks ran, in order, with DP_BUNDLE_DIR and DP_MODE set"
    );

    // Away and back: the ledger remembers, so nothing is appended a second time.
    let b = env.bundle_b();
    env.run(&["use", b.to_str().unwrap(), "-y"]);
    env.run(&["use", "shared-rice", "-y"]);
    assert_eq!(
        std::fs::read_to_string(&log).unwrap().lines().count(),
        2,
        "hooks run on first activation only"
    );

    env.run(&["use", "-", "-y"]);
    env.run(&["use", "shared-rice", "-y", "--run-hooks"]);
    assert_eq!(
        std::fs::read_to_string(&log).unwrap().lines().count(),
        4,
        "--run-hooks forces them"
    );

    // The same source again collides on the manifest's name, and `--as` is the way out.
    let out = env.try_run(&["add", &url]);
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--as"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    env.run(&["add", &url, "--as", "second-copy"]);
    assert!(env.store("second-copy").join("dotfiles.toml").is_file());
    assert!(
        !env.store("second-copy").join("config/shared").is_symlink(),
        "add downloads and installs nothing"
    );
}

/// A repo that is not a bundle is rejected, and the clone it came from is not left
/// behind — there is deliberately no fallback that runs a foreign `install.sh`.
#[test]
fn a_repo_without_a_manifest_is_refused() {
    let env = TestEnv::new("no-manifest");
    let repo = env.root.join("plain-repo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("install.sh"), "#!/bin/sh\necho pwned\n").unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "first"]);

    let out = env.try_run(&["add", &format!("file://{}", repo.display())]);
    let said = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success(), "{said}");
    assert!(said.contains("not a dotpack bundle"), "{said}");
    assert!(
        !env.home
            .join(".local/share/dotpack/bundles/.fetching")
            .exists(),
        "and the clone was thrown away"
    );
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "user.email=t@example.com", "-c", "user.name=test"])
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {}\n{}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
}
