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
    # Before C, the command line as run (OSC 633 E), which tTerm cannot
    # rebuild from the keys when it came from history or completion.
    # Backslashes and the control characters that would end the sequence are
    # escaped as `\\` and `\xHH`; splitting on lines and joining them escapes
    # the line breaks.
    function __tterm_mark_output --on-event fish_preexec
        set -l line (string replace -a '\\' '\\\\' -- $argv[1])
        set line (string join '\\x0a' -- $line | string replace -a \r '\\x0d' \
            | string replace -a \t '\\x09' | string replace -a \e '\\x1b' \
            | string replace -a \a '\\x07')
        printf '\e]633;E;%s\e\\\\\e]133;C\e\\\\' "$line"
    end
    function __tterm_mark_done --on-event fish_postexec
        printf '\e]133;D;%s\e\\\\' $status
    end
end
set -e TTERM_COMMAND_MARKS
