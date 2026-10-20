// Command marks (OSC 133) and working directory reports (OSC 7) for shells on
// remote hosts, which tTerm does not change. The user adds one of these to
// their shell's startup file; they send the same marks and reports as tTerm's
// local scripts (src-tauri/src/terminal/shell_integration), but not the command
// line (OSC 633 E), so tTerm rebuilds it from the keys. The directory lets the
// SFTP drawer follow the shell; only `%` needs escaping in it, as tTerm reads
// the rest of the path as sent. `${"$"}` writes the `$` of a shell expansion
// that would otherwise start a placeholder here.

/** For the end of ~/.bashrc or ~/.zshrc. */
export const POSIX_SHELL_INTEGRATION = String.raw`# tTerm: mark prompts and commands (OSC 133) to jump between commands and
# copy their output, and report the directory (OSC 7) for SFTP to follow.
# Keep at the end, after prompt themes.
if [ -z "$__tterm_marks" ]; then
  __tterm_marks=1
  if [ -n "$ZSH_VERSION" ]; then
    __tterm_mark_prompt() { printf '\033]133;D;%s\033\\\033]7;file://%s\033\\\033]133;A\033\\' "$?" "${"$"}{PWD//\%/%25}"; }
    __tterm_mark_output() { printf '\033]133;C\033\\'; }
    autoload -Uz add-zsh-hook
    precmd_functions=(__tterm_mark_prompt $precmd_functions)
    add-zsh-hook preexec __tterm_mark_output
  elif [ -n "$BASH_VERSION" ]; then
    __tterm_mark_prompt() { local s=$?; printf '\033]133;D;%s\033\\\033]7;file://%s\033\\\033]133;A\033\\' "$s" "${"$"}{PWD//\%/%25}"; return $s; }
    if [ -n "$PROMPT_COMMAND" ]; then
      PROMPT_COMMAND="__tterm_mark_prompt;$PROMPT_COMMAND"
    else
      PROMPT_COMMAND=__tterm_mark_prompt
    fi
    PS0="$PS0"'\e]133;C\e\\'
  fi
fi
`

/** For ~/.config/fish/config.fish. */
export const FISH_SHELL_INTEGRATION = String.raw`# tTerm: mark prompts and commands (OSC 133) to jump between commands and
# copy their output, and report the directory (OSC 7) for SFTP to follow.
if status is-interactive; and not set -q __tterm_marks
    set -g __tterm_marks 1
    function __tterm_mark_prompt --on-event fish_prompt
        printf '\e]7;file://%s\e\\\\' (string replace --all '%' '%25' -- $PWD)
        printf '\e]133;A\e\\\\'
    end
    function __tterm_mark_output --on-event fish_preexec
        printf '\e]133;C\e\\\\'
    end
    function __tterm_mark_done --on-event fish_postexec
        printf '\e]133;D;%s\e\\\\' $status
    end
end
`
