# Starts the user's shell as a login shell with tTerm's shell integration
# from $tterm_dir: it reports its working directory (OSC 7) at every prompt
# and, unless $tterm_marks is 0, marks its prompts and commands (OSC 133).
# wsl/launch.sh sources it; an SSH session runs
# `exec sh <dir>/unix/start.sh --motd`.
[ -n "${tterm_dir:-}" ] || tterm_dir=${0%/*}/..
tterm_marks=${tterm_marks:-1}

# sshd shows the login banner only when asked for a plain shell, so an SSH
# session prints it here: the dynamic part pam_motd builds on Debian and
# Ubuntu, then /etc/motd. ~/.hushlogin turns it off, as it does for sshd.
if [ "${1:-}" = --motd ] && [ ! -e "$HOME/.hushlogin" ]; then
    for tterm_motd in /run/motd.dynamic /etc/motd; do
        [ -r "$tterm_motd" ] && cat "$tterm_motd"
    done
fi

shell=${SHELL:-}
[ -x "$shell" ] || shell=$(getent passwd "$(id -un)" 2>/dev/null | cut -d: -f7)
[ -x "$shell" ] || shell=/bin/sh

case "${shell##*/}" in
    bash)
        # An exported function plus PROMPT_COMMAND, as for Git Bash: bash
        # stays a plain login shell. PS0 marks where output starts.
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
