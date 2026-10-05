# tTerm launcher for WSL, started as `wsl.exe --cd <shell-integration dir>
# -e sh ./wsl/launch.sh <restore dir> <fallback Windows dir> <marks>`. It
# changes into the directory to restore, then starts the user's shell through
# unix/start.sh, marking commands (OSC 133) when <marks> is 1.
tterm_dir=$PWD
tterm_marks=${3:-0}

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

. "$tterm_dir/unix/start.sh"
