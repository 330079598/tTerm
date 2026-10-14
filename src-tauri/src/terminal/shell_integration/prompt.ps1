# tTerm shell integration for Windows PowerShell and PowerShell 7, passed as
# plain text to -Command so it runs after the user profile. Wraps the prompt so
# every prompt reports the working directory (OSC 9;9) and, when
# TTERM_COMMAND_MARKS is set, marks where each prompt starts and how the last
# command ended (OSC 133 A and D), and with PSReadLine where each command's
# output starts (C) and the command line itself (OSC 633 E). Contains no
# double quotes, which would need escaping on the Windows command line.
if (-not $global:__TTermPromptInstalled) {
    $global:__TTermPromptInstalled = $true
    $global:__TTermOriginalPrompt = $function:prompt
    $global:__TTermCommandMarks = [bool]$env:TTERM_COMMAND_MARKS
    $global:__TTermLastHistoryId = -1
    Remove-Item Env:TTERM_COMMAND_MARKS -ErrorAction Ignore
    # Reports each command line as run (OSC 633 E), which tTerm cannot rebuild
    # from the keys when it came from history or completion, and where its
    # output starts (OSC 133 C). Backslashes and control characters, which
    # would end the sequence, are escaped as \xHH. Wraps PSReadLine's line
    # editor, so the prompt sets it up once PSReadLine has loaded; without
    # PSReadLine tTerm takes Enter as the start of the output.
    function global:__TTermWrapReadLine {
        if ($global:__TTermOriginalReadLine -or -not (Test-Path Function:\PSConsoleHostReadLine)) { return }
        $global:__TTermOriginalReadLine = $function:PSConsoleHostReadLine
        function global:PSConsoleHostReadLine {
            $tTermLine = & $global:__TTermOriginalReadLine
            if (-not [string]::IsNullOrWhiteSpace($tTermLine)) {
                $tTermEsc = [string][char]27
                $tTermEscaped = [regex]::Replace($tTermLine, '[\\\x00-\x1f\x7f]', { '\x{0:x2}' -f [int][char]$args[0].Value })
                [Console]::Write($tTermEsc + ']633;E;' + $tTermEscaped + $tTermEsc + '\' + $tTermEsc + ']133;C' + $tTermEsc + '\')
            }
            $tTermLine
        }
    }
    function global:prompt {
        # Read first: every statement resets $?.
        $tTermFailed = -not $global:?
        $tTermEsc = [string][char]27
        $tTermMarks = ''
        if ($global:__TTermCommandMarks) {
            __TTermWrapReadLine
            # Enter on an empty line adds no history entry and ran nothing.
            $tTermLast = Get-History -Count 1
            if ($tTermLast -and $tTermLast.Id -ne $global:__TTermLastHistoryId) {
                $global:__TTermLastHistoryId = $tTermLast.Id
                $tTermMarks = $tTermEsc + ']133;D;' + [int]$tTermFailed + $tTermEsc + '\'
            }
            $tTermMarks += $tTermEsc + ']133;A' + $tTermEsc + '\'
        }
        # Sets $? back so the original prompt still sees the last command's.
        if ($tTermFailed) { Write-Error failure -ErrorAction Ignore }
        $tTermPrompt = & $global:__TTermOriginalPrompt
        $tTermLocation = $executionContext.SessionState.Path.CurrentLocation
        if ($tTermLocation.Provider.Name -ne 'FileSystem') { return $tTermMarks + $tTermPrompt }
        $tTermMarks + $tTermEsc + ']9;9;' + [char]34 + $tTermLocation.ProviderPath + [char]34 + $tTermEsc + '\' + $tTermPrompt
    }
}
