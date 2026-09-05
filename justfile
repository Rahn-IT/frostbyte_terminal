set shell := ["bash", "-euo", "pipefail", "-c"]

# list commands also default
list:
    @just --list

# Build the GitHub release in a clean Arch chroot.
build:
    pkgctl build

# Install exactly the package paths reported by makepkg, keeping build logs.
install: build
    makepkg --packagelist | xargs -r -d '\n' sudo pacman -U --noconfirm

# Run after publishing the GitHub tag matching frostbyte_term/Cargo.toml.
pkg-update:
    version=$(sed -n '/^version = /{s/.*"\(.*\)".*/\1/p;q;}' frostbyte_term/Cargo.toml); \
    test -n "$version"; \
    sed -i "s/^pkgver=.*/pkgver=$version/" PKGBUILD
    checksums=$(makepkg --geninteg -p PKGBUILD | grep '^sha256sums='); \
    sed -i "s/^sha256sums=.*/$checksums/" PKGBUILD
    makepkg --printsrcinfo > .SRCINFO

run:
    cargo run --release
