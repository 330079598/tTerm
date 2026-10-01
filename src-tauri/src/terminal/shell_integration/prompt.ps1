# tTerm shell integration for Windows PowerShell and PowerShell 7, passed as
# plain text to -Command so it runs after the user profile. Wraps the prompt so
# every prompt reports the working directory (OSC 9;9). Contains no double
# quotes, which would need escaping on the Windows command line.
if (-not $global:__TTermPromptInstalled) {
    $global:__TTermPromptInstalled = $true
    $global:__TTermOriginalPrompt = $function:prompt
    function global:prompt {
        # Runs first so the original prompt still sees $? from the last command.
        $tTermPrompt = & $global:__TTermOriginalPrompt
        $tTermLocation = $executionContext.SessionState.Path.CurrentLocation
        if ($tTermLocation.Provider.Name -ne 'FileSystem') { return $tTermPrompt }
        $tTermEsc = [string][char]27
        $tTermEsc + ']9;9;' + [char]34 + $tTermLocation.ProviderPath + [char]34 + $tTermEsc + '\' + $tTermPrompt
    }
}
