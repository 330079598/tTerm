#!/usr/bin/env bash
# Runs one pi round for a GPUI migration card, either a fresh session in the
# card's worktree or a resumed one, and keeps the event log and the last
# message in the dispatch directory. See docs/gpui-migration/dispatch.md.
#
#   scripts/migration/run-pi.sh CARD WORKTREE PROMPT_FILE [SESSION_ID]
#
# pi has no sandbox of its own, so it runs under a macOS Seatbelt profile:
# writes are limited to the worktree, the repository's .git, cargo's and
# pnpm's caches, pi's own state and the temporary directories, and the two
# data directories that hold stone's real connections cannot be read or
# written at all. Everything else (network, processes, ptys) stays allowed,
# so tests that need a pty run without asking for anything.
#
# A new session gets the id CARD-STAMP; pass that id as SESSION_ID to resume
# it in a later round. The dispatch directory defaults to ../tTerm-dispatch
# next to the main checkout; set TTERM_DISPATCH_DIR to put it elsewhere, and
# PI_MODEL to use another model.
set -euo pipefail

if [[ $# -lt 3 || $# -gt 4 ]]; then
  echo "usage: $0 CARD WORKTREE PROMPT_FILE [SESSION_ID]" >&2
  exit 2
fi
if [[ $(uname) != Darwin ]]; then
  echo "run-pi.sh needs macOS sandbox-exec; use run-codex on other systems" >&2
  exit 2
fi
card=$1
worktree=$(cd "$2" && pwd -P)
prompt=$(cd "$(dirname "$3")" && pwd -P)/$(basename "$3")
session=${4:-}

git_common=$(cd "$(git -C "$worktree" rev-parse --path-format=absolute --git-common-dir)" && pwd -P)
main_checkout=$(dirname "$git_common")
dispatch_dir=${TTERM_DISPATCH_DIR:-"$(dirname "$main_checkout")/tTerm-dispatch"}
mkdir -p "$dispatch_dir"
dispatch_dir=$(cd "$dispatch_dir" && pwd -P)
stamp=$(date +%Y%m%d-%H%M%S)
log="$dispatch_dir/$card-$stamp.jsonl"
last="$dispatch_dir/$card-$stamp.last.md"
session_dir="$dispatch_dir/pi-sessions/$card"
mkdir -p "$session_dir"
if [[ -z $session ]]; then session="$card-$stamp"; fi
model=${PI_MODEL:-magpie/sunline-ais/deepseek-v4.1-flash}

home=$(cd "$HOME" && pwd -P)
cargo_home=$(cd "${CARGO_HOME:-$home/.cargo}" && pwd -P)
tmp=$(cd "${TMPDIR:-/tmp}" && pwd -P)
profile="$dispatch_dir/pi-sandbox-$card.sb"
cat >"$profile" <<EOF
(version 1)
(allow default)
(deny file-write*)
(allow file-write*
  (subpath "$worktree")
  (subpath "$git_common")
  (subpath "$session_dir")
  (subpath "$cargo_home")
  (subpath "$home/.rustup")
  (subpath "$home/.pi")
  (subpath "$home/Library/pnpm")
  (subpath "$home/Library/Caches")
  (subpath "$home/.cache")
  (subpath "$home/.local/share/pnpm")
  (subpath "$home/.local/state")
  (subpath "$tmp")
  (subpath "/private/tmp")
  (subpath "/private/var/folders")
  (subpath "/dev"))
(deny file-read* file-write*
  (subpath "$home/Library/Application Support/com.stone.tTerm")
  (subpath "$home/Library/Application Support/com.stone.tTerm.dev")
  (subpath "$home/Library/Caches/com.stone.tTerm")
  (subpath "$home/Library/Caches/com.stone.tTerm.dev")
  (subpath "$home/.config/tterm"))
EOF

cd "$worktree"
status=0
sandbox-exec -f "$profile" pi -p --mode json \
  --no-extensions --no-mcp --no-prompt-templates --no-themes \
  --model "$model" \
  --session-dir "$session_dir" --session-id "$session" \
  "$(cat "$prompt")" </dev/null >"$log" 2>"$log.stderr" || status=$?

# The last assistant message, for the coordinator to read first.
python3 -I - "$log" >"$last" <<'PY' || true
import json, sys
last = None
for line in open(sys.argv[1], encoding="utf-8"):
    try:
        event = json.loads(line)
    except ValueError:
        continue
    if event.get("type") == "message_end" and event.get("message", {}).get("role") == "assistant":
        last = event["message"]
if last:
    print("\n".join(part.get("text", "") for part in last.get("content", []) if part.get("type") == "text"))
PY
echo "card=$card exit=$status log=$log last=$last session=$session"
exit "$status"
