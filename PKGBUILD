# Maintainer: Roman <neciplay25@gmail.com>
pkgname=cross-cleaner-cli
pkgver=1.9.6
pkgrel=1
pkgdesc="A powerful system cleanup tool written in Rust"
arch=('x86_64')
url="https://github.com/WinBooster/Cross-Cleaner"
license=('GPL-3.0')
depends=()
makedepends=('cargo' 'git')
source=("git+$url.git#tag=v$pkgver")
sha256sums=('SKIP')

build() {
  cd "$srcdir"
  cargo build --release
}

package() {
  cd "$srcdir/${pkgname}"
  install -Dm755 "target/release/Cross_Cleaner_CLI" "$pkgdir/usr/bin/${pkgname}"
  # Установите дополнительные файлы (man pages, .desktop и т.д.)
}