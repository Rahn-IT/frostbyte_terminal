

# list commands also default
list:
    @just --list

install:
    rm -f frostbyte_terminal-*.tar.zst
    rm -f frostbyte_terminal-*.log
    pkgctl build
    sudo pacman -U --noconfirm frostbyte_terminal-*.pkg.tar.zst
    rm -f frostbyte_terminal-*.tar.zst
    rm -f frostbyte_terminal-*.log

pkg-update:
    sed -i "s/^pkgver=.*/pkgver=$(grep '^version' frostbyte_term/Cargo.toml | head -1 | cut -d '"' -f2)/" PKGBUILD
    sed -i "s/^sha256sums=.*/$(makepkg --geninteg -p PKGBUILD | grep "^sha256sums=")/" PKGBUILD

run:
    cargo run --release
