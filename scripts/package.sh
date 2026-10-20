#!/usr/bin/env bash
# Packages the desktop app for the system it runs on, into dist-desktop/:
#   macOS:   MX-Shooting-Calculator-<version>-macos.dmg (Apple silicon and Intel)
#   Windows: MX-Shooting-Calculator-<version>-windows-x64.zip
#   Linux:   MX-Shooting-Calculator-<version>-linux-x64.tar.gz
# The release workflow (.github/workflows/desktop.yml) runs it on each system.
set -euo pipefail
cd "$(dirname "$0")/.."

# rustup's toolchain: Homebrew's own Rust can't build for the other Mac.
if command -v brew >/dev/null 2>&1 && [ -d "$(brew --prefix rustup 2>/dev/null)/bin" ]; then
    PATH="$(brew --prefix rustup)/bin:$PATH"
fi

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
name="MX Shooting Calculator"
file="MX-Shooting-Calculator-$version"
out=dist-desktop
work=target/package
licences=(LICENSE-MIT LICENSE-APACHE NOTICE THIRD-PARTY-LICENSES.txt)
rm -rf "$out" "$work"
mkdir -p "$out" "$work/icons"

# The icons, drawn from the SVG.
cargo run --release --quiet -p mxsc --example icons -- "$work/icons"

case "$(uname -s)" in
Darwin)
    rustup target add aarch64-apple-darwin x86_64-apple-darwin
    for target in aarch64-apple-darwin x86_64-apple-darwin; do
        MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release -p mxsc --target "$target"
    done
    app="$work/dmg/$name.app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    # One program for both kinds of Mac.
    lipo -create -output "$app/Contents/MacOS/mxsc" \
        target/aarch64-apple-darwin/release/mxsc target/x86_64-apple-darwin/release/mxsc
    iconutil -c icns -o "$app/Contents/Resources/mxsc.icns" "$work/icons/mxsc.iconset"
    sed "s/@VERSION@/$version/g" crates/app/packaging/Info.plist >"$app/Contents/Info.plist"
    cp "${licences[@]}" "$app/Contents/Resources/"
    # Apple silicon runs only signed programs: sign it without an identity
    # (not notarized, so the first start needs Open Anyway).
    codesign --force --deep --sign - "$app"
    ln -s /Applications "$work/dmg/Applications"
    hdiutil create -volname "$name" -srcfolder "$work/dmg" -ov -format UDZO \
        "$out/$file-macos.dmg"
    ;;
MINGW* | MSYS* | CYGWIN*)
    MXSC_ICO="$(cygpath -w "$PWD/$work/icons/mxsc.ico")" cargo build --release -p mxsc
    dir="$work/$file"
    mkdir -p "$dir"
    cp target/release/mxsc.exe "$dir/$name.exe"
    cp "${licences[@]}" "$dir/"
    (cd "$work" && 7z a -tzip "../../$out/$file-windows-x64.zip" "$file" >/dev/null)
    ;;
Linux)
    cargo build --release -p mxsc
    dir="$work/$file"
    mkdir -p "$dir"
    cp target/release/mxsc "$dir/"
    cp "$work/icons/mxsc-256.png" "$dir/mxsc.png"
    cp crates/app/packaging/mxsc.desktop crates/app/packaging/install.sh "$dir/"
    cp "${licences[@]}" "$dir/"
    tar -czf "$out/$file-linux-x64.tar.gz" -C "$work" "$file"
    ;;
*)
    echo "Don't know how to package for $(uname -s)" >&2
    exit 1
    ;;
esac
ls -l "$out"
