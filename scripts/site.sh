#!/usr/bin/env bash
# The download page (site/) into the web build, at download/, with this
# version's download links. scripts/web.sh build runs it after the app.
#
#   scripts/site.sh [web build folder]   (default crates/app/dist)
set -euo pipefail
cd "$(dirname "$0")/.."

dist=${1:-crates/app/dist}
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
out="$dist/download"
mkdir -p "$out/images"
for page in site/*.html; do
    sed "s/@VERSION@/$version/g" "$page" >"$out/$(basename "$page")"
done
cp site/images/* "$out/images/"
cp crates/app/assets/mxsc-icon.svg crates/app/assets/mxsc-wordmark.svg "$out/"
# The export file's schema, at the address the schema gives as its $id.
cp docs/export-format.schema.json "$out/"
echo "Download page for $version in $out"
