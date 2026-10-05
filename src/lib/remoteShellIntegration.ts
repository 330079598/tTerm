// Command marks (OSC 133) for shells on remote hosts, which tTerm does not
// change. The user adds one of these to their shell's startup file; they send
// the same marks as tTerm's local scripts (src-tauri/src/terminal/shell_integration).

/** For the end of ~/.bashrc or ~/.zshrc. */
export const POSIX_SHELL_INTEGRATION = String.raw`# tTerm: mark prompts and commands (OSC 133) to jump between commands and
# copy their output. Keep at the end, after prompt themes.
if [ -z "$__tterm_marks" ]; then
  __tterm_marks=1
  if [ -n "$ZSH_VERSION" ]; then
    __tterm_mark_prompt() { printf '\033]133;D;%s\033\\\033]133;A\033\\' "$?"; }
    __tterm_mark_output() { printf '\033]133;C\033\\'; }
    autoload -Uz add-zsh-hook
    precmd_functions=(__tterm_mark_prompt $precmd_functions)
    add-zsh-hook preexec __tterm_mark_output
  elif [ -n "$BASH_VERSION" ]; then
    __tterm_mark_prompt() { local s=$?; printf '\033]133;D;%s\033\\\033]133;A\033\\' "$s"; return $s; }
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
# copy their output.
if status is-interactive; and not set -q __tterm_marks
    set -g __tterm_marks 1
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
`
