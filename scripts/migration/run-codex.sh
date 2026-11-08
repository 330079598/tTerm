#!/usr/bin/env bash
# Runs one Codex round for a GPUI migration card, either a fresh session in
# the card's worktree or a resumed one, and keeps the event log and the last
# message in the dispatch directory. See docs/gpui-migration/dispatch.md.
#
#   scripts/migration/run-codex.sh CARD WORKTREE PROMPT_FILE [THREAD_ID]
#
# The dispatch directory defaults to ../tTerm-dispatch next to the main
# checkout; set TTERM_DISPATCH_DIR to put it elsewhere.
set -euo pipefail

if [[ $# -lt 3 || $# -gt 4 ]]; then
  echo "usage: $0 CARD WORKTREE PROMPT_FILE [THREAD_ID]" >&2
  exit 2
fi
card=$1
worktree=$(cd "$2" && pwd)
prompt=$3
resume=${4:-}

# Commits made in a worktree land in the main repository's .git, so that
# directory must be writable from the sandbox, along with cargo's cache.
git_common=$(git -C "$worktree" rev-parse --path-format=absolute --git-common-dir)
main_checkout=$(dirname "$git_common")
dispatch_dir=${TTERM_DISPATCH_DIR:-"$(dirname "$main_checkout")/tTerm-dispatch"}
mkdir -p "$dispatch_dir"
stamp=$(date +%Y%m%d-%H%M%S)
log="$dispatch_dir/$card-$stamp.jsonl"
last="$dispatch_dir/$card-$stamp.last.md"
cargo_home=${CARGO_HOME:-"$HOME/.cargo"}

# `--approve-for-me` refuses an explicit -s, and `exec resume` takes neither
# -C nor -s, so every sandbox setting goes in as config. The automatic
# reviewer answers escalation requests instead of a person.
config=(
  -c 'sandbox_mode="workspace-write"'
  -c 'approval_policy="on-request"'
  -c 'approvals_reviewer="auto_review"'
  -c 'sandbox_workspace_write.network_access=true'
  -c "sandbox_workspace_write.writable_roots=['$git_common','$cargo_home']"
)

cd "$worktree"
status=0
if [[ -n $resume ]]; then
  codex exec resume "$resume" "${config[@]}" --json -o "$last" - <"$prompt" >"$log" 2>&1 || status=$?
else
  codex exec -C "$worktree" "${config[@]}" --json -o "$last" - <"$prompt" >"$log" 2>&1 || status=$?
fi

thread=$(grep -o '"thread_id":"[^"]*"' "$log" | head -n 1 | cut -d'"' -f4 || true)
echo "card=$card exit=$status log=$log last=$last thread=$thread"
exit "$status"
