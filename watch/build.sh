#!/usr/bin/env bash
# Builds the watch app for the epix (Gen 2).
#
#   ./build.sh          bin/mxsc-watch.prg, to sideload or run in the simulator
#   ./build.sh test     builds with the unit tests and runs them in the simulator
#   ./build.sh run      builds and runs the app in the simulator
#   ./build.sh package  bin/mxsc-watch.iq, for the Connect IQ Store (also as
#                       a beta app), for every watch in manifest.xml
#
# DEVICE=... picks another device the SDK Manager has downloaded.
set -euo pipefail
cd "$(dirname "$0")"

# The compiler is Java.
export PATH="/opt/homebrew/opt/openjdk@17/bin:$PATH"

CIQ="$HOME/Library/Application Support/Garmin/ConnectIQ"
if [ ! -f "$CIQ/current-sdk.cfg" ]; then
    echo "No Connect IQ SDK: install one with the SDK Manager (see README.md)." >&2
    exit 1
fi
SDK="$(tr -d '\r\n' < "$CIQ/current-sdk.cfg")"
SDK="${SDK%/}"
KEY="$CIQ/developer_key.der"
DEVICE="${DEVICE:-epix2}"
MODE="${1:-build}"

if [ ! -f "$KEY" ]; then
    echo "No developer key at $KEY (see README.md)." >&2
    exit 1
fi
if [ "$MODE" != package ] && [ ! -d "$CIQ/Devices/$DEVICE" ]; then
    echo "The SDK has no $DEVICE yet: download it in the SDK Manager (see README.md)." >&2
    exit 1
fi

mkdir -p bin

# The simulator must be running for monkeydo.
simulator() {
    if ! pgrep -qf "ConnectIQ.app/Contents/MacOS"; then
        open "$SDK/bin/ConnectIQ.app"
        sleep 5
    fi
}

case "$MODE" in
    build)
        "$SDK/bin/monkeyc" -f monkey.jungle -d "$DEVICE" -y "$KEY" -w -o bin/mxsc-watch.prg
        echo "Built bin/mxsc-watch.prg for $DEVICE."
        ;;
    run)
        "$SDK/bin/monkeyc" -f monkey.jungle -d "$DEVICE" -y "$KEY" -w -o bin/mxsc-watch.prg
        simulator
        "$SDK/bin/monkeydo" bin/mxsc-watch.prg "$DEVICE"
        ;;
    test)
        "$SDK/bin/monkeyc" -f monkey.jungle -d "$DEVICE" -y "$KEY" -w -t -o bin/mxsc-watch-test.prg
        simulator
        "$SDK/bin/monkeydo" bin/mxsc-watch-test.prg "$DEVICE" -t
        ;;
    package)
        "$SDK/bin/monkeyc" -f monkey.jungle -e -r -y "$KEY" -w -o bin/mxsc-watch.iq
        echo "Built bin/mxsc-watch.iq."
        ;;
    *)
        echo "usage: ./build.sh [build|run|test|package]" >&2
        exit 2
        ;;
esac
