# Maintainer: habitui contributors
#
# In-tree PKGBUILD: run `makepkg -si` from the project root.
# It uses source=() and copies the working tree from $startdir into the
# build directory, so it does not require a tarball or git remote.
#
# To build from a release tarball instead, replace source=() and the
# prepare() body with something like:
#   source=("$pkgname-$pkgver.tar.gz::https://example.invalid/.../v$pkgver.tar.gz")
#   sha256sums=('SKIP')
# and remove the cp -a calls below.
pkgname=habitui
pkgver=0.1.0
pkgrel=1
pkgdesc="Terminal UI habit tracker written in Rust"
arch=('x86_64')
url="https://example.invalid/habitui"
license=('MIT')
depends=('gcc-libs' 'glibc')
makedepends=('cargo' 'rust')
source=()
sha256sums=()

prepare() {
    # Mirror the working tree into $srcdir, excluding build artifacts.
    rm -rf "$srcdir/$pkgname-$pkgver"
    mkdir -p "$srcdir/$pkgname-$pkgver"
    cp -a "$startdir/Cargo.toml" "$startdir/Cargo.lock" "$startdir/src" \
        "$srcdir/$pkgname-$pkgver/"
    if [ -d "$startdir/tests" ]; then
        cp -a "$startdir/tests" "$srcdir/$pkgname-$pkgver/"
    fi
    if [ -f "$startdir/LICENSE" ]; then
        cp -a "$startdir/LICENSE" "$srcdir/$pkgname-$pkgver/"
    fi
}

build() {
    cd "$srcdir/$pkgname-$pkgver"
    export CARGO_TARGET_DIR=target
    cargo build --release --locked
}

check() {
    cd "$srcdir/$pkgname-$pkgver"
    export CARGO_TARGET_DIR=target
    cargo test --release --locked
}

package() {
    cd "$srcdir/$pkgname-$pkgver"
    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
    if [ -f LICENSE ]; then
        install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    fi
}
