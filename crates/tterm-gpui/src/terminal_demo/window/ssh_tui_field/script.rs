//! The fixed field script and its per-stage markers.
//!
//! Every marker is produced by `printf 'P416-%s\n' NAME`, never typed as one
//! literal string, so the echoed command line cannot look like completed
//! output. The shell works only inside the private temporary `HOME` created
//! by the first stage.

/// One non-interactive shell step with its completion markers.
pub(super) struct ShellStage {
    pub name: &'static str,
    pub detail: &'static str,
    pub command: &'static str,
    pub sentinel: &'static str,
    pub ok: &'static str,
    pub fail: &'static str,
    pub required: &'static [&'static str],
    pub screenshot: &'static str,
}

pub(super) const LEFT_MARKER: &str = "P416-LEFT";
pub(super) const RIGHT_MARKER: &str = "P416-RIGHT";
pub(super) const NVIM_ORIGINAL: &str = "P416-ORIGINAL";

pub(super) const TMUX_ZOOM: &[u8] = b"\x02z";
/// The private server uses tmux's default prefix-r binding for refresh-client.
pub(super) const TMUX_REDRAW: &[u8] = b"\x02r";
pub(super) const TMUX_DETACH: &[u8] = b"\x02d";
pub(super) const NVIM_QUIT: &[u8] = b":wq\r";
pub(super) const NVIM_FORCE_QUIT: &[u8] = b"\x1b:qa!\r";

/// Creates the private directory tree and replaces the login shell with a
/// clean POSIX `sh`, so later stages do not depend on zsh word splitting or a
/// user startup file.
pub(super) const ENTER_POSIX_SHELL: &str = r#"printf 'P416-%s\n' S0; P416_ROOT=$(mktemp -d /tmp/tterm-field.XXXXXXXX) && case "$P416_ROOT" in /tmp/tterm-field.*) export P416_ROOT;; *) exit 9;; esac && umask 077 && export HOME="$P416_ROOT" TMUX_TMPDIR="$P416_ROOT/tmux" P416_FILE="$P416_ROOT/field.txt" P416_MARKER="$P416_ROOT/.p416-field" P416_SOCKET="$P416_ROOT/tmux.sock" P416_ORIGINAL=P416-ORIGINAL P416_EDIT=P416-NVIM-EDIT && mkdir -p "$TMUX_TMPDIR" && printf '%s\n' "$P416_ROOT" > "$P416_MARKER" && cd "$P416_ROOT" && exec env HISTFILE=/dev/null ENV=/dev/null SHELL=/bin/sh PATH=/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:/snap/bin /bin/sh -i"#;

pub(super) struct KeyStep {
    pub bytes: &'static [u8],
    pub wait_ms: u64,
}

/// Inserts one line, leaves insert mode, undoes the change and redoes it.
pub(super) const NVIM_EDIT_STEPS: &[KeyStep] = &[
    KeyStep {
        bytes: b"ggO",
        wait_ms: 200,
    },
    KeyStep {
        bytes: b"P416-NVIM-EDIT",
        wait_ms: 200,
    },
    KeyStep {
        bytes: b"\x1b",
        wait_ms: 300,
    },
    KeyStep {
        bytes: b"u",
        wait_ms: 300,
    },
    KeyStep {
        bytes: b"\x12",
        wait_ms: 300,
    },
];

pub(super) fn shell_ready() -> ShellStage {
    ShellStage {
        name: "shell-ready",
        detail: "A private HOME exists and the remote shell is a clean POSIX sh.",
        command: r#"printf 'P416-%s\n' S1; case "$0" in *sh) printf 'P416-%s\n' POSIX;; *) printf 'P416-%s\n' POSIXFAIL;; esac; case "$P416_ROOT" in /tmp/tterm-field.*) printf 'P416-%s\n' PRIVATE;; *) printf 'P416-%s\n' PRIVATEFAIL;; esac; p416_verify_root() { case "$P416_ROOT" in /tmp/tterm-field.*) ;; *) return 1;; esac; [ -n "$P416_MARKER" ] && [ -f "$P416_MARKER" ] || return 1; [ "$(cat "$P416_MARKER")" = "$P416_ROOT" ] || return 1; [ "$(find "$P416_ROOT" -prune -user "$(id -un)" -print 2>/dev/null)" = "$P416_ROOT" ] || return 1; [ "$P416_SOCKET" = "$P416_ROOT/tmux.sock" ] || return 1; return 0; }; p416_cleanup() { if p416_verify_root; then tmux_field kill-server >/dev/null 2>&1; rm -f "$P416_FILE" "$P416_SOCKET" "$P416_MARKER"; rmdir "$TMUX_TMPDIR" "$P416_ROOT" >/dev/null 2>&1; fi; }; readonly P416_ROOT P416_FILE P416_MARKER P416_SOCKET; tmux_field() { tmux -f /dev/null -S "$P416_SOCKET" "$@"; }; trap 'cd /; p416_cleanup' 0 HUP INT TERM; printf 'P416-%s\n' "$SHELL"; command -v tmux >/dev/null 2>&1 && command -v nvim >/dev/null 2>&1 && test -d "$TMUX_TMPDIR" && printf 'P416-%s\n' M1 || printf 'P416-%s\n' M1FAIL"#,
        sentinel: "P416-S1",
        ok: "P416-M1",
        fail: "P416-M1FAIL",
        required: &["P416-POSIX", "P416-PRIVATE", "P416-/bin/sh"],
        screenshot: "02-shell-ready.png",
    }
}

pub(super) fn tmux_split() -> ShellStage {
    ShellStage {
        name: "tmux-split",
        detail: "tmux created a detached 80x24 session, split it into two panes, and both panes printed their marker.",
        command: r#"printf 'P416-%s\n' S2; P416_L=P416-$(printf LEFT); export P416_L; P416_R=P416-$(printf RIGHT); export P416_R; tmux_field new-session -d -s field -x 80 -y 24 && tmux_field split-window -h -t field && tmux_field send-keys -t field.0 "X=\$(printf LEFT); printf 'P416-%s\\n' \"\$X\"" Enter && tmux_field send-keys -t field.1 "X=\$(printf RIGHT); printf 'P416-%s\\n' \"\$X\"" Enter && sleep 2 && test "$(tmux_field display-message -p -t field '#{window_panes}')" = 2 && tmux_field capture-pane -p -t field.0 | grep -qF "$P416_L" && tmux_field capture-pane -p -t field.1 | grep -qF "$P416_R" && printf 'P416-%s\n' M2 || printf 'P416-%s\n' M2FAIL"#,
        sentinel: "P416-S2",
        ok: "P416-M2",
        fail: "P416-M2FAIL",
        required: &[],
        screenshot: "02b-tmux-created.png",
    }
}

pub(super) fn tmux_attach() -> ShellStage {
    ShellStage {
        name: "tmux-attach",
        detail: "",
        command: r#"printf 'P416-%s\n' S3; sleep 0.4; tmux_field attach -t field && printf 'P416-%s\n' M3 || printf 'P416-%s\n' M3FAIL"#,
        sentinel: "P416-S3",
        ok: "P416-M3",
        fail: "P416-M3FAIL",
        required: &[],
        screenshot: "",
    }
}

pub(super) fn tmux_verify_close() -> ShellStage {
    ShellStage {
        name: "tmux-verify-close",
        detail: "Both pane buffers still held their markers after detach, and killing the session left no tmux server.",
        command: r#"printf 'P416-%s\n' S4; if tmux_field capture-pane -p -t field.0 | grep -qF "$P416_L" && tmux_field capture-pane -p -t field.1 | grep -qF "$P416_R" && test "$(tmux_field display-message -p -t field '#{window_panes}')" = 2 && tmux_field kill-session -t field && ! tmux_field has-session -t field >/dev/null 2>&1; then printf 'P416-%s\n' M4; else printf 'P416-%s\n' M4FAIL; fi"#,
        sentinel: "P416-S4",
        ok: "P416-M4",
        fail: "P416-M4FAIL",
        required: &[],
        screenshot: "05b-tmux-closed.png",
    }
}

pub(super) fn nvim_launch() -> ShellStage {
    ShellStage {
        name: "nvim",
        detail: "",
        command: r#"printf 'P416-%s\n' S5; printf '%s\n' "$P416_ORIGINAL" > "$P416_FILE" && sleep 0.4 && nvim --clean "$P416_FILE" && printf 'P416-%s\n' M5 || printf 'P416-%s\n' M5FAIL"#,
        sentinel: "P416-S5",
        ok: "P416-M5",
        fail: "P416-M5FAIL",
        required: &[],
        screenshot: "",
    }
}

pub(super) fn nvim_verify() -> ShellStage {
    ShellStage {
        name: "nvim-verify",
        detail: "The file contains both the original line and the line that was inserted, undone and redone in nvim.",
        command: r#"printf 'P416-%s\n' S6; if grep -qxF "$P416_EDIT" "$P416_FILE" && grep -qxF "$P416_ORIGINAL" "$P416_FILE"; then cat "$P416_FILE"; printf 'P416-%s\n' M6; else printf 'P416-%s\n' M6FAIL; fi"#,
        sentinel: "P416-S6",
        ok: "P416-M6",
        fail: "P416-M6FAIL",
        required: &[NVIM_ORIGINAL, "P416-NVIM-EDIT"],
        screenshot: "09-nvim-file.png",
    }
}

pub(super) fn final_shell() -> ShellStage {
    ShellStage {
        name: "final-shell",
        detail: "The shell prompt is back on the primary screen.",
        command: r#"printf 'P416-%s\n' S7; printf 'P416-%s\n' "$SHELL" && printf 'P416-%s\n' M7"#,
        sentinel: "P416-S7",
        ok: "P416-M7",
        fail: "P416-M7FAIL",
        required: &["P416-/bin/sh"],
        screenshot: "10-shell-restored.png",
    }
}

pub(super) fn cleanup() -> ShellStage {
    ShellStage {
        name: "cleanup",
        detail: "tmux was stopped and the private temporary HOME was removed.",
        command: r#"printf 'P416-%s\n' S8; cd /; p416_cleanup; if [ -n "$P416_MARKER" ] && [ ! -e "$P416_FILE" ] && [ ! -e "$P416_SOCKET" ] && [ ! -e "$P416_MARKER" ]; then printf 'P416-%s\n' CLEAN; else printf 'P416-%s\n' CLEANFAIL; fi"#,
        sentinel: "P416-S8",
        ok: "P416-CLEAN",
        fail: "P416-CLEANFAIL",
        required: &[],
        screenshot: "",
    }
}
