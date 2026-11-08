#!/usr/bin/env bash
# Runs inside the terminal under measurement, never through SSH.
set -euo pipefail
load=${1:?cat|yes|vim|tmux|vtebench}
result=${2:?absolute result directory}
mkdir -p "$result"
text="$result/text-50MB.txt"
if [[ ! -f "$text" ]] || [[ $(wc -c < "$text") != 50000000 ]]; then
  python3 - "$text" <<'PY'
import sys
line = ("tTerm P0-01 deterministic ASCII workload " + "0123456789" * 6)[:99] + "\n"
assert len(line.encode()) == 100
with open(sys.argv[1], "wb") as target:
    chunk = line.encode() * 10000
    for _ in range(50):
        target.write(chunk)
PY
fi
date -u +%FT%TZ > "$result/$load.started"
case "$load" in
  prepare) sha256sum "$text" ;;
  cat) cat "$text" ;;
  yes)
    timeout 30s yes 'tTerm P0-01 yes 0123456789' || [[ $? == 124 ]]
    ;;
  vim)
    # Vim's own timer scrolls the visible viewport, preserving real screen redraws.
    cat > "$result/scroll.vim" <<'VIM'
set nomore noswapfile nowrap lazyredraw&
let g:perf_ticks = 0
function! PerfScroll(timer)
  execute "normal! \<C-E>"
  redraw
  let g:perf_ticks += 1
  if g:perf_ticks >= 1500
    call timer_stop(a:timer)
    qa!
  endif
endfunction
call timer_start(20, 'PerfScroll', {'repeat': -1})
VIM
    vim -Nu NONE -i NONE -n -S "$result/scroll.vim" "$text"
    ;;
  tmux)
    socket="tterm-perf-$$"
    trap 'tmux -L "$socket" kill-server 2>/dev/null || true' EXIT
    tmux -L "$socket" new-session -d -x "${COLUMNS:-120}" -y "${LINES:-30}" \
      "timeout 30s yes 'pane-1 0123456789'; sleep 3"
    for pane in 2 3 4; do
      tmux -L "$socket" split-window "timeout 30s yes 'pane-$pane 0123456789'; sleep 3"
      tmux -L "$socket" select-layout tiled >/dev/null
    done
    tmux -L "$socket" attach-session
    ;;
  vtebench)
    bench=${VTEBENCH_DIR:-$HOME/vtebench}
    if [[ ! -x "$bench/target/release/vtebench" ]]; then
      printf '%s\n' '待补（WSL 缺少 Rust 工具链）' >&2
      exit 2
    fi
    cd "$bench"
    ./target/release/vtebench --dat "$result/vtebench.dat"
    ;;
  *) printf 'Unknown workload: %s\n' "$load" >&2; exit 2 ;;
esac
date -u +%FT%TZ > "$result/$load.done"
