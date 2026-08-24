# The bundle format — schema 1

A **bundle** is a directory that carries a rice: the config files, the packages they need,
and a description of what the result is made of. One git repo = one bundle.

The format exists because the description already exists everywhere and is thrown away
every time. Every r/unixporn post lists its bar, its terminal, its font and its theme —
in prose, in a comment, unreadable by anything. The repo it links to holds none of it.
This is that list, moved into a file next to the files it describes.

| Document | What it specifies |
|---|---|
| this file | the directory layout, where each file lands, and what an installer must do |
| [manifest.md](./manifest.md) | `dotfiles.toml` — every field, its type, and the validation rules |
| [components.md](./components.md) | the role vocabulary: `bar`, `terminal`, `icons`, `font_system`… |

Three things are needed to read this: a bundle is a directory, `dotfiles.toml` is its only
mandatory file, and everything else is convention.

**Version.** `schema = 1`, and this document is it. An implementation that meets a higher
number warns and keeps going — a manifest is mostly still readable across one version of
anything.

**On the references.** `design.md`, `real-world.md` and `TODO.md` are named in a few
places for provenance. They are the reference implementation's own working notes, they
live outside this directory, and nothing in them is part of the standard.

---

## Layout

```
awesome-rice/
├── dotfiles.toml         # the manifest — the only mandatory file
├── README.md             # human-readable summary; the reference implementation generates it
├── config/               → ~/.config/
│   ├── hypr/
│   ├── waybar/
│   └── kitty/
├── home/                 → ~/
│   ├── .bashrc
│   └── .gitconfig
├── local/                → ~/.local/
│   ├── bin/
│   └── share/
│       ├── fonts/
│       └── applications/
├── assets/               → wherever `dotfiles.toml` says (wallpapers and the like)
│   └── wallpapers/
└── hooks/                → scripts run at install time, optional
    ├── pre-install.sh
    └── post-install.sh
```

**Where a file lands is implied by the directory it sits in.** The manifest holds no file
list:

| Path in the bundle | Destination |
|---|---|
| `config/<X>` | `~/.config/<X>` — at the depth the rule below picks |
| `home/<X>` | `~/<X>` |
| `local/<X>` | `~/.local/<X>` |
| `assets/<X>` | nowhere, unless `dotfiles.toml` declares a destination |

Which buys three things: the manifest stays small enough to hand-write, adding a file is
copying it into a directory and nothing else, and somebody browsing the repo on GitHub can
see what they are getting without running anything. `assets/` is the exception because
there is no convention for where a wallpaper goes.

---

## Link rules

Rices do not all take over a config directory. Some install *underneath* one, alongside
config the user wrote themselves. One rule covers both:

> Walk down from `config/`. If a directory **contains files directly**, that directory is
> the link and the walk stops. If it holds only directories, descend and repeat.

| Bundle content | What is placed |
|---|---|
| `config/hypr/hyprland.conf` | `~/.config/hypr` |
| `config/hypr/themes/cyberpunk/theme.conf`, nothing above it | `~/.config/hypr/themes/cyberpunk` |

**The depth rule is `config/`'s alone. `home/` and `local/` are per file, always.** `~`,
`~/.local/bin` and `~/.local/share/fonts` are mixed directories — they hold things that
belong to the user and to no bundle, hand-installed Nerd Fonts being the common case — and
a directory link hides all of them.

| Area | Granularity |
|---|---|
| `config/` | directory, at the depth rule's depth |
| `home/`, `local/` | per file |

Placing `~/.config/hypr/themes/cyberpunk` may require creating `~/.config/hypr/themes/`
first. An implementation that creates a directory this way records it and removes it again
when the bundle is deactivated **if it is empty** — otherwise switching leaves litter.

---

## What an installer must do

The rules an author is entitled to assume, whatever tool the receiver uses:

1. **Nothing is destroyed without a backup.** A real file at a target path is moved
   somewhere recoverable, not overwritten.
2. **Executable bits survive.** A rice ships dozens of scripts; a lost `+x` breaks it
   silently. Copy files, never read-then-write.
3. **Hooks run on a bundle's first activation only**, and their source is shown to the
   user before it runs. Real hooks append to files, and appending twice is not undoable.
   `pre_install` runs before packages, `post_install` after the files are in place.
4. **A `url` in `components` is never fetched.** It means "the user installs this by
   hand" and is printed as a step. Anything that must arrive with the bundle ships inside
   it.
5. **Packages are only installed, never removed.** Switching away from a bundle leaves
   what it brought in; removing it is the user's decision and nobody else's.
6. **`ignore` is collect-time only.** It keeps a path out of the bundle. It cannot skip a
   file at install time — `config/hypr` is one link, and a config that `source`s an
   ignored file is broken for the receiver.
7. **A directory without `dotfiles.toml` is not a bundle**, and is rejected as one.
   Running a foreign `install.sh` instead is exactly what this format exists to end.
8. **`assets` are copied and nothing more.** Never linked, never removed on the way out,
   and never written over a file that is already at the dest — that dest is a directory
   the user owns, so a bundle shipping `forest.png` cannot cost anyone theirs.

---

## Reference implementation and example

[dotpack](https://github.com/murat-akpinar/dotpack) implements all of the above; its
`example/` directory is a real rice in this format, with a hand-written README explaining
every awkward entry in its manifest. Neither is normative. The three documents here are.
