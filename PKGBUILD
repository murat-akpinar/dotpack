# Maintainer: Murat AKPINAR <akpinarmurat@protonmail.com>

# Arch is the only target, so this is how the tool arrives: `pacman -R dotpack-git`
# removes it, which a `cargo install` into ~/.cargo/bin does not.
pkgname=dotpack-git
pkgver=r0.0000000
pkgrel=1
pkgdesc='Dotfiles, bundled with the packages they need'
arch=('x86_64')
url='https://github.com/murat-akpinar/dotpack'
license=('GPL-3.0-or-later')
# pacman, systemd and coreutils are in `base`, so they are not listed. git is not, and
# `use github:…` is a shell-out to it; fc-cache is fontconfig's, and a font nothing has
# indexed does not exist to a running application.
depends=('git' 'fontconfig')
makedepends=('git' 'cargo')
optdepends=(
  'paru: AUR packages in a bundle'
  'yay: the same, if you use yay'
  'wl-clipboard: `dotpack post` copies the list to the clipboard, on wayland'
  'xclip: the same, on X'
)
provides=('dotpack')
conflicts=('dotpack')
source=("git+$url.git")
sha256sums=('SKIP')

pkgver() {
  cd dotpack
  printf 'r%s.%s' "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
}

prepare() {
  cd dotpack
  export RUSTUP_TOOLCHAIN=stable
  cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
  cd dotpack
  export RUSTUP_TOOLCHAIN=stable CARGO_TARGET_DIR=target
  cargo build --frozen --release
}

check() {
  cd dotpack
  export RUSTUP_TOOLCHAIN=stable
  # The switch test stubs pacman, sudo and systemctl on PATH and runs against a
  # temporary HOME, so a build server is never touched (invariant 14).
  cargo test --frozen
}

package() {
  cd dotpack
  install -Dm755 target/release/dotpack "$pkgdir/usr/bin/dotpack"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
