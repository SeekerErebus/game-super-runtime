#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Expected a numeric release version' >&2; exit 1; }
repository=${GSR_REPOSITORY_URL:-}
if [[ -n $repository && ! $repository =~ ^https://[a-zA-Z0-9./_-]+$ ]]; then
    echo 'GSR_REPOSITORY_URL must be an HTTPS repository URL without query parameters' >&2
    exit 1
fi
name="game-super-runtime-$version"
archive="$name.tar.gz"
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
mkdir -p "$stage/$name" target/dist
cp -R Cargo.toml Cargo.lock LICENSE README.md Makefile src assets docs examples tests tools packaging "$stage/$name/"
# SOURCE_DATE_EPOCH permits byte-reproducible tarballs; default is a fixed epoch.
tar --sort=name --mtime="@${SOURCE_DATE_EPOCH:-0}" --owner=0 --group=0 --numeric-owner -C "$stage" -czf "target/dist/$archive" "$name"
checksum=$(sha256sum "target/dist/$archive")
checksum=${checksum%% *}
source_name=$archive
if [[ -n $repository ]]; then source_name="$repository/releases/download/v$version/$archive"; fi
sed -e "s|@VERSION@|$version|g" -e "s|@REPOSITORY_URL@|$repository|g" \
    -e "s|@SOURCE@|$source_name|g" -e "s|@SHA256@|$checksum|g" \
    packaging/PKGBUILD.in > target/dist/PKGBUILD
printf '%s  %s\n' "$checksum" "$archive" > target/dist/SHA256SUMS
if command -v makepkg >/dev/null; then
    (cd target/dist && makepkg --printsrcinfo > .SRCINFO)
fi
printf 'Created target/dist/%s and checksummed PKGBUILD.\n' "$archive"
