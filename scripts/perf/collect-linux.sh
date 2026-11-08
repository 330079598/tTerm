#!/usr/bin/env bash
# Not run on a native Linux desktop in P0-01.
set -euo pipefail
exec python3 "$(dirname "$0")/collect-unix.py" "$@"
