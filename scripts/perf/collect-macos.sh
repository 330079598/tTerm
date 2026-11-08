#!/usr/bin/env bash
# Not run on macOS in this Windows-only P0-01 session.
set -euo pipefail
exec python3 "$(dirname "$0")/collect-unix.py" "$@"
