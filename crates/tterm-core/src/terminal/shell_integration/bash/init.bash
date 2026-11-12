# tTerm shell integration for bash, read as $ENV by `bash --posix --login`
# from unix/start.sh. In POSIX mode bash reads $ENV and no other startup file,
# so this leaves that mode, reads the login files as bash would and only then
# adds tTerm's hook: one set up front is lost to a profile that assigns
# PROMPT_COMMAND outright. The shell stays a login shell for `shopt
# login_shell`, `logout` and ~/.bash_logout.
__tterm_bash_dir=${ENV%/*}
unset ENV
builtin set +o posix
# Leaving POSIX mode leaves this on (bash 4.4 and later).
builtin shopt -u inherit_errexit 2>/dev/null
# start.sh exported HISTFILE only to keep POSIX mode's ~/.sh_history away.
if [[ -n ${TTERM_UNEXPORT_HISTFILE-} ]]; then
    builtin export -n HISTFILE
    unset TTERM_UNEXPORT_HISTFILE
fi

# The login files, in bash's order (run_startup_files in bash's shell.c).
[[ -r /etc/profile ]] && builtin source /etc/profile
if [[ -r ~/.bash_profile ]]; then
    builtin source ~/.bash_profile
elif [[ -r ~/.bash_login ]]; then
    builtin source ~/.bash_login
elif [[ -r ~/.profile ]]; then
    builtin source ~/.profile
fi

# The prompt hook reports the working directory (OSC 7) and, unless command
# marks are off in tTerm's settings, how the last command ended and where the
# prompt starts (OSC 133 D and A). It runs first, while $? is still the
# command's. PS0 marks where each command's output starts (C). The hook's
# body is the one Git Bash gets as an exported function.
if [[ -n ${TTERM_COMMAND_MARKS-} ]]; then
    builtin eval "__tterm_prompt $(<"$__tterm_bash_dir/prompt.bash")"
    PS0=${PS0-}'\e]133;C\e\\'
else
    builtin eval "__tterm_prompt $(<"$__tterm_bash_dir/report-cwd.bash")"
fi
PROMPT_COMMAND=__tterm_prompt${PROMPT_COMMAND:+$'\n'$PROMPT_COMMAND}
unset TTERM_COMMAND_MARKS __tterm_bash_dir
