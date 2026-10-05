# tTerm shell integration for fish under WSL, sourced through --init-command
# after the user's own configuration.
set -e TTERM_FISH_INIT

# Report the working directory before every prompt (OSC 7). Only `%` needs
# escaping for tTerm's parser.
function __tterm_report_cwd --on-event fish_prompt
    printf '\e]7;file://%s\e\\\\' (string replace --all '%' '%25' -- $PWD)
end

# Command marks (OSC 133), unless turned off in tTerm's settings: where the
# prompt starts (A), where a command's output starts (C), and how it ended (D).
if set -q TTERM_COMMAND_MARKS
    function __tterm_mark_prompt --on-event fish_prompt
        printf '\e]133;A\e\\\\'
    end
    function __tterm_mark_output --on-event fish_preexec
        printf '\e]133;C\e\\\\'
    end
    function __tterm_mark_done --on-event fish_postexec
        printf '\e]133;D;%s\e\\\\' $status
    end
end
set -e TTERM_COMMAND_MARKS
