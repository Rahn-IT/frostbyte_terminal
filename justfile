

# list commands also default
list:
    @just --list

pkg-install:
    pkgctl build

pkg-update:
    sed -i "s/^pkgver=.*/pkgver=$(grep '^version' frostbyte_term/Cargo.toml | head -1 | cut -d '"' -f2)/" PKGBUILD
    sed -i "s/^sha256sums=.*/$(makepkg --geninteg -p PKGBUILD | grep "^sha256sums=")/" PKGBUILD

run:
    cargo run --release
