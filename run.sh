#!/usr/bin/env bash
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"

if [ $# -eq 0 ]; then
    # Default: launch the bot
    cargo run --bin bot
else
    cargo run --bin lwsc2 -- "$@"
fi
