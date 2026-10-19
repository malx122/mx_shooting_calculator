#!/usr/bin/env bash
# Starts the MCP server for AI assistants (crates/mxsc-mcp), rebuilt first
# when the code has changed, so an assistant always gets the current
# version. Register this script with the assistant instead of a fixed
# binary, e.g.:
#
#   claude mcp add --scope user mxsc -- "$PWD/scripts/mxsc-mcp.sh" --db "$PWD/target/dev.sqlite"
#
# Arguments go to mxsc-mcp (such as --db <file>; a relative path is taken
# from the repository root). MCP talks over stdout, so the build writes to
# stderr only.
set -euo pipefail
cd "$(dirname "$0")/.."

# Assistants started from the Dock may not have the shell's PATH.
PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

cargo build --quiet --release -p mxsc-mcp 1>&2
exec target/release/mxsc-mcp "$@"
