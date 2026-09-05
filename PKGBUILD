# Maintainer: acul009 <acul009@gmail.com>
pkgname=frostbyte_terminal
pkgver=0.2.8
pkgrel=1
pkgdesc="A rust based cross platform dropdown terminal inspired by yakuake"
arch=('x86_64')
url="https://github.com/Rahn-IT/frostbyte_terminal"
license=('MIT')
depends=('hicolor-icon-theme' 'glibc' 'gdk-pixbuf2' 'glib2' 'xdotool' 'gtk3' 'libgcc')
makedepends=('cargo' 'rust' 'cairo' 'atk')
options=('!debug')
source=("$pkgname-$pkgver.tar.gz::$url/archive/v$pkgver.tar.gz")
sha256sums=('ca212bd1d4ced9831b45ef71d0fa5c816f80631e387320d31bb33884fd634713')

build() {
  cd "$pkgname-$pkgver"
  cargo build --release --locked
}

package() {
  cd "$pkgname-$pkgver"
  install -Dm755 "target/release/frostbyte_term" "$pkgdir/usr/bin/$pkgname"
  install -Dm644 "frostbyte_term/assets/icon.png" "$pkgdir/usr/share/icons/hicolor/512x512/apps/$pkgname.png"
  install -Dm644 "frostbyte_term/assets/frostbyte_terminal.desktop" -t "$pkgdir/usr/share/applications"
  # Install any additional files (e.g., licenses, configs) here
}
