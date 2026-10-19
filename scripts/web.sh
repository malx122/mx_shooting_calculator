#!/usr/bin/env bash
# The web version of MX Shooting Calculator.
#
#   scripts/web.sh          serve it on http://127.0.0.1:8080, rebuilding on changes
#   scripts/web.sh build    release build into crates/app/dist (static files to host)
#
# Needs rustup with the wasm32-unknown-unknown target, trunk, and llvm-ar
# (macOS: `brew install rustup trunk llvm`, then
#  `rustup toolchain install stable --target wasm32-unknown-unknown`).
set -euo pipefail
cd "$(dirname "$0")/../crates/app"

# rustup's toolchain: Homebrew's own Rust can't build for the browser.
if command -v brew >/dev/null 2>&1 && [ -d "$(brew --prefix rustup 2>/dev/null)/bin" ]; then
  PATH="$(brew --prefix rustup)/bin:$PATH"
fi
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-stable}"

# SQLite is C code compiled for the browser; macOS's ar can't index those
# objects, so use LLVM's.
if [ -z "${AR_wasm32_unknown_unknown:-}" ]; then
  for candidate in "$(command -v llvm-ar 2>/dev/null || true)" \
                   "$(brew --prefix llvm 2>/dev/null)/bin/llvm-ar"; do
    if [ -n "$candidate" ] && [ -x "$candidate" ]; then
      export AR_wasm32_unknown_unknown="$candidate"
      break
    fi
  done
fi
if [ -z "${AR_wasm32_unknown_unknown:-}" ]; then
  echo "llvm-ar not found: install LLVM (macOS: brew install llvm)" >&2
  exit 1
fi

case "${1:-serve}" in
  serve) exec trunk serve ;;
  build) exec trunk build --release --public-url ./ ;;
  *) echo "usage: $0 [serve|build]" >&2; exit 2 ;;
esac
