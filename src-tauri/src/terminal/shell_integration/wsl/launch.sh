# tTerm launcher for WSL, started as `wsl.exe --cd <shell-integration dir>
# -e sh ./wsl/launch.sh <restore dir> <fallback Windows dir> <marks>`. It
# changes into the directory to restore, then starts the user's shell as a
# login shell that reports its working directory (OSC 7) at every prompt and,
# when <marks> is 1, marks its prompts and commands (OSC 133).
tterm_dir=$PWD
tterm_marks=${3:-}

# Some sh treat `cd ""` as a successful no-op, so empty paths fail here.
tterm_cd() {
    [ -n "$1" ] && cd -- "$1" 2>/dev/null
}

# The restored directory is a Linux path, or a Windows one when the tab used
# another shell before.
tterm_cd "$1" ||
    tterm_cd "$(wslpath -u "$1" 2>/dev/null)" ||
    tterm_cd "$(wslpath -u "$2" 2>/dev/null)" ||
    cd

shell=${SHELL:-}
[ -x "$shell" ] || shell=$(getent passwd "$(id -un)" 2>/dev/null | cut -d: -f7)
[ -x "$shell" ] || shell=/bin/sh

case "${shell##*/}" in
    bash)
        # An exported function plus PROMPT_COMMAND, as for Git Bash: bash
        # stays a plain login shell. Windows' PROMPT_COMMAND never reaches
        # WSL, so there is none to keep. PS0 marks where output starts.
        if [ "$tterm_marks" = 1 ] && [ -r "$tterm_dir/bash/prompt.bash" ]; then
            exec env "BASH_FUNC___tterm_prompt%%=$(cat "$tterm_dir/bash/prompt.bash")" \
                PROMPT_COMMAND=__tterm_prompt 'PS0=\e]133;C\e\\' "$shell" -l
        fi
        if [ -r "$tterm_dir/bash/report-cwd.bash" ]; then
            exec env "BASH_FUNC___tterm_report_cwd%%=$(cat "$tterm_dir/bash/report-cwd.bash")" \
                PROMPT_COMMAND=__tterm_report_cwd "$shell" -l
        fi
        ;;
    zsh)
        if [ -r "$tterm_dir/zsh/.zshrc" ]; then
            [ "$tterm_marks" = 1 ] && export TTERM_COMMAND_MARKS=1
            export TTERM_USER_ZDOTDIR="${ZDOTDIR:-}"
            export ZDOTDIR="$tterm_dir/zsh"
            exec "$shell" -l
        fi
        ;;
    fish)
        if [ -r "$tterm_dir/fish/tterm.fish" ]; then
            [ "$tterm_marks" = 1 ] && export TTERM_COMMAND_MARKS=1
            export TTERM_FISH_INIT="$tterm_dir/fish/tterm.fish"
            exec "$shell" -l -C 'source $TTERM_FISH_INIT'
        fi
        ;;
esac

case "${shell##*/}" in
    sh | dash | ash | ksh | mksh | bash | zsh | fish) exec "$shell" -l ;;
    *) exec "$shell" ;;
esac
