#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
	printf 'Usage: %s <Scoopy.deb> <output-directory>\n' "$0" >&2
	exit 1
fi

if [[ $(id -u) -eq 0 ]]; then
	printf 'Build the Arch package as an ordinary user, as required by makepkg.\n' >&2
	exit 1
fi

deb="$(realpath "$1")"
mkdir -p "$2"
output="$(realpath "$2")"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

control_member="$(bsdtar -tf "$deb" | awk '/^control\.tar(\.|$)/ { print; exit }')"
data_member="$(bsdtar -tf "$deb" | awk '/^data\.tar(\.|$)/ { print; exit }')"
if [[ ! "$control_member" =~ ^control\.tar(\.(gz|xz|zst|bz2|lzma))?$ || ! "$data_member" =~ ^data\.tar(\.(gz|xz|zst|bz2|lzma))?$ ]]; then
	printf 'The input is not a Debian package with control and data archives.\n' >&2
	exit 1
fi

mkdir "$work/control"
bsdtar -xOf "$deb" "$control_member" | bsdtar -xf - -C "$work/control"
version="$(awk '/^Version:/ { print $2 }' "$work/control/control")"
architecture="$(awk '/^Architecture:/ { print $2 }' "$work/control/control")"
if [[ ! "$version" =~ ^[0-9][0-9A-Za-z.+~_-]*$ ]]; then
	printf 'Unsupported package version: %s\n' "$version" >&2
	exit 1
fi

case "$architecture" in
	amd64) architecture=x86_64 ;;
	arm64) architecture=aarch64 ;;
	*) printf 'Unsupported package architecture: %s\n' "$architecture" >&2; exit 1 ;;
esac

cp "$deb" "$work/Scoopy.deb"
checksum="$(sha256sum "$work/Scoopy.deb" | cut -d ' ' -f 1)"
cat > "$work/PKGBUILD" <<EOF
pkgname=scoopy-bin
pkgver=${version//-/_}
pkgrel=1
pkgdesc="Screen recording with Studio, Instant, and screenshot modes"
arch=("$architecture")
url="https://example.com"
license=("AGPL-3.0-only")
depends=("libva" "libpulse" "libpipewire" "alsa-lib" "alsa-plugins" "libxkbcommon" "libxkbcommon-x11" "libx11" "openssl" "wayland" "libglvnd" "vulkan-icd-loader" "xdg-desktop-portal" "xdg-utils")
optdepends=("xdg-desktop-portal-hyprland: screen capture on Hyprland and Omarchy" "vulkan-driver: hardware accelerated recording and editing")
provides=("cap=\$pkgver")
conflicts=("scoopy")
options=("!strip" "!debug")
source=("Scoopy.deb")
noextract=("Scoopy.deb")
sha256sums=("$checksum")

package() {
	bsdtar -xOf "\$srcdir/Scoopy.deb" "$data_member" | bsdtar -xf - -C "\$pkgdir"
	for binary in Scoopy scoopy-cli scoopy-muxer; do
		test -x "\$pkgdir/usr/bin/\$binary"
	done
	printf 'arch\n' > "\$pkgdir/usr/lib/scoopy/package-format"
}
EOF

cd "$work"
PKGDEST="$output" makepkg --nodeps --noconfirm
