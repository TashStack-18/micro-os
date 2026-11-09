#!/usr/bin/env bash
# Build the kernel and start the web control panel on http://localhost:8080
#
#   scripts/frontend.sh                       defaults: Round Robin, quantum 4
#   scripts/frontend.sh --port 9000 --policy mlfq
#
# Stop it with Ctrl-C. Open the URL in a Windows browser: WSL forwards localhost.
set -euo pipefail
# Make cargo/rustc available even from a non-login shell.
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
(cd "$ROOT/kernel" && cargo build --release --quiet)
exec python3 "$ROOT/frontend/server.py" "$@"
