# Runs one Codex round for a GPUI migration card on Windows, either a fresh
# session in the card's worktree or a resumed one, and keeps the event log and
# the last message in the dispatch directory. See docs/gpui-migration/dispatch.md.
#
#   scripts/migration/run-codex.ps1 -Card ID -Worktree PATH -PromptFile FILE [-ResumeId THREAD]
#
# The dispatch directory defaults to ..\tTerm-dispatch next to the main
# checkout; set TTERM_DISPATCH_DIR to put it elsewhere.
param(
    [Parameter(Mandatory)] [string] $Card,
    [Parameter(Mandatory)] [string] $Worktree,
    [Parameter(Mandatory)] [string] $PromptFile,
    [string] $ResumeId
)

$ErrorActionPreference = 'Stop'
$Worktree = (Resolve-Path $Worktree).Path
$PromptFile = (Resolve-Path $PromptFile).Path

# Commits made in a worktree land in the main repository's .git, so that
# directory must be writable from the sandbox, along with cargo's cache.
$gitCommon = (Resolve-Path (git -C $Worktree rev-parse --path-format=absolute --git-common-dir)).Path
$mainCheckout = Split-Path -Parent $gitCommon
$dir = if ($env:TTERM_DISPATCH_DIR) { $env:TTERM_DISPATCH_DIR } else { Join-Path (Split-Path -Parent $mainCheckout) 'tTerm-dispatch' }
New-Item -ItemType Directory -Force $dir | Out-Null
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$log = Join-Path $dir "$Card-$stamp.jsonl"
$last = Join-Path $dir "$Card-$stamp.last.md"
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
# The user's PowerShell profile runs fnm, which writes a shim directory on
# every shell start.
$fnm = Join-Path $env:LOCALAPPDATA 'fnm_multishells'

# `--approve-for-me` refuses an explicit -s, and `exec resume` takes neither
# -C nor -s, so every sandbox setting goes in as config. The sandbox keeps
# .git read-only and blocks some child processes on Windows, so commits and
# test runners need an escalation, which the automatic reviewer answers
# instead of a person.
$config = @(
    '-c', 'sandbox_mode="workspace-write"',
    '-c', 'approval_policy="on-request"',
    '-c', 'approvals_reviewer="auto_review"',
    '-c', 'sandbox_workspace_write.network_access=true',
    '-c', "sandbox_workspace_write.writable_roots=['$gitCommon','$cargoHome','$fnm']"
)

Push-Location $Worktree
try {
    if ($ResumeId) {
        Get-Content $PromptFile -Raw | codex exec resume $ResumeId @config --json -o $last - *> $log
    } else {
        Get-Content $PromptFile -Raw | codex exec -C $Worktree @config --json -o $last - *> $log
    }
    $status = $LASTEXITCODE
} finally {
    Pop-Location
}

$thread = Select-String -Path $log -Pattern '"thread_id":"([^"]+)"' | Select-Object -First 1
"card=$Card exit=$status log=$log last=$last thread=$($thread.Matches.Groups[1].Value)"
