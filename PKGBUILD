# Maintainer: acul009 <acul009@gmail.com>
pkgname=frostbyte_terminal
pkgver=0.2.6 # Match your Cargo.toml version
pkgrel=1
pkgdesc="A rust based cross platform dropdown terminal inspired by yakuake"
arch=('x86_64')
url="https://github.com/Rahn-IT/frostbyte_terminal"
license=('MIT') # Or your license
depends=()      # List runtime dependencies here
makedepends=('cargo' 'rust')
source=("$pkgname-$pkgver.tar.gz::$url/archive/v$pkgver.tar.gz")
sha256sums=('a92fd185aa1455130ce2083d2b4786352641488d31b1dbb8e78aca98f8944491')

build() {
  ls
  cd "$pkgname-$pkgver"
  cargo build --release --locked
}

package() {
  cd "$pkgname-$pkgver"
  install -Dm755 "target/release/frostbyte_term" -t "$pkgdir/usr/bin"
  install -Dm644 "frostbyte_term/assets/icon.png" "$pkgdir/usr/share/icons/hicolor/256x256/apps/$pkgname.png"
  install -Dm644 "frostbyte_term/assets/frostbyte-terminal.desktop" -t "$pkgdir/usr/share/applications"
  # Install any additional files (e.g., licenses, configs) here
}
