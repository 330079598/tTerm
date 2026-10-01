# tTerm shell integration for fish under WSL, sourced through --init-command
# after the user's own configuration.
set -e TTERM_FISH_INIT

# Report the working directory before every prompt (OSC 7). Only `%` needs
# escaping for tTerm's parser.
function __tterm_report_cwd --on-event fish_prompt
    printf '\e]7;file://%s\e\\\\' (string replace --all '%' '%25' -- $PWD)
end
