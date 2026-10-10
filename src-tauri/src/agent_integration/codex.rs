//! Codex: hooks in `hooks.json` under `$CODEX_HOME`.
//!
//! Codex starts hooks in a session of their own, without a controlling
//! terminal, and has no field to write a sequence for them. So each hook
//! finds the terminal itself. Run on its own, Codex is the first of the hook's
//! ancestors with one. By default it hands its sessions to a shared background
//! server, and the hook descends from that server instead: it then writes to
//! every terminal running Codex, with the session's id, for tTerm to keep the
//! report in the tab the session was submitted from. Codex runs a hook only
//! once the user has trusted it in `/hooks`, and again after it changes.

use super::hooks::HookSet;

/// `Interrupt` (Esc or Ctrl+C) leaves Codex waiting for the user without
/// finishing anything.
pub(super) const HOOKS: HookSet = HookSet {
    file: "hooks.json",
    events: &[
        ("UserPromptSubmit", "processing"),
        ("PostToolUse", "processing"),
        ("PermissionRequest", "waiting"),
        ("Stop", "done"),
        ("Interrupt", "idle"),
        ("SessionEnd", "ended"),
    ],
    command: hook_command,
};

/// A POSIX script under `sh -c`, so it runs the same whatever login shell
/// Codex starts it with. Its escapes are printf's: fish would read `\\` in
/// single quotes as one backslash, so the string terminator's is `\134`.
/// One `ps` lists the processes for awk, which prints each terminal to write
/// to, whether a tmux pane holds it, and whether the report goes there
/// unaddressed (Codex's own terminal) or with the session's id (every Codex
/// terminal). tmux passes the report on only wrapped, and with
/// `allow-passthrough` on.
fn hook_command(state: &str) -> String {
    let script = [
        r#"sid=$(sed -n "s/.*\"session_id\"[[:space:]]*:[[:space:]]*\"\([^\"]*\)\".*/\1/p" | head -n 1 | tr -cd A-Za-z0-9-)"#,
        &format!(
            r#"ps -A -o pid= -o ppid= -o tty= -o comm= 2>/dev/null | awk -v h=$PPID "{AWK}" | while read x t b; do r="codex;{state}"; if [ "$b" = 1 ]; then [ -n "$sid" ] || continue; r="$r;$sid"; fi; s="\033]777;tterm-agent;$r\007"; [ "$x" = 0 ] || s="\033Ptmux;\033$s\033\134"; {{ printf "$s" > "/dev/$t"; }} 2>/dev/null; done"#
        ),
    ]
    .join("; ");
    format!("sh -c '{script}'")
}

/// awk in the script's double quotes: `\$` for its fields, `\"` for its
/// strings. `?` or `??` stands for no terminal; tmux's server is `tmux` or
/// `tmux: server`.
const AWK: &str = concat!(
    r#"function has(p) { return t[p] != \"\" && t[p] !~ /^[?]/ } "#,
    r#"function emit(p, b,  q, x) { if (t[p] in s) return; s[t[p]] = 1; x = 0; for (q = p; q > 1; q = pp[q]) if (n[q] ~ /^tmux/) x = 1; print x, t[p], b } "#,
    r#"{ pp[\$1] = \$2; t[\$1] = \$3; c = \$4; for (i = 5; i <= NF; i++) c = c \" \" \$i; sub(/.*\//, \"\", c); n[\$1] = c } "#,
    r#"END { for (p = h; p > 1 && !has(p); p = pp[p]); if (p > 1) emit(p, 0); else for (p in n) if (n[p] == \"codex\" && has(p)) emit(p, 1) }"#,
);

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn script(command: &str) -> &str {
        command
            .strip_prefix("sh -c '")
            .and_then(|rest| rest.strip_suffix('\''))
            .unwrap()
    }

    #[test]
    fn hook_is_one_single_quoted_posix_script() {
        let command = hook_command("waiting");
        let script = script(&command);
        assert!(!script.contains('\''));
        let status = std::process::Command::new("sh")
            .args(["-n", "-c", script])
            .status()
            .unwrap();
        assert!(status.success());
    }

    /// Runs the hook against a process table from a stand-in `ps`, whose
    /// terminals are files in a scratch directory. Returns what each got.
    fn run_hook(state: &str, stdin: &str, table: &str) -> Vec<(String, String)> {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("tterm-codex-hook-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        // `/dev/../<dir>/<name>` is the file.
        let tty = format!("..{}", dir.display());
        let me = std::process::id();
        let table = table.replace("ME", &me.to_string()).replace("TTY", &tty);
        std::fs::write(dir.join("table"), table).unwrap();
        let ps = dir.join("ps");
        std::fs::write(
            &ps,
            format!("#!/bin/sh\ncat '{}'\n", dir.join("table").display()),
        )
        .unwrap();
        std::fs::set_permissions(&ps, std::fs::Permissions::from_mode(0o755)).unwrap();

        let command = hook_command(state);
        let mut child = std::process::Command::new("sh")
            .args(["-c", script(&command)])
            .env(
                "PATH",
                format!("{}:{}", dir.display(), std::env::var("PATH").unwrap()),
            )
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.as_bytes())
            .unwrap();
        assert!(child.wait().unwrap().success());

        let mut written: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.starts_with("tty"))
            .map(|name| {
                let text = std::fs::read_to_string(dir.join(&name)).unwrap();
                (name, text)
            })
            .collect();
        written.sort();
        std::fs::remove_dir_all(&dir).unwrap();
        written
    }

    const INPUT: &str = r#"{"session_id":"01a12490-4b68-76d0","turn_id":"t","tool_input":{"command":"echo \"session_id\":\"x\""}}"#;

    #[test]
    fn hook_writes_to_the_codex_it_runs_under() {
        // The hook under Codex in tmux; another Codex elsewhere.
        let table = "ME 500 ?? zsh\n\
                     500 400 TTY/ttyA codex\n\
                     400 300 TTY/ttyA zsh\n\
                     300 1 ?? tmux\n\
                     600 1 TTY/ttyB /opt/homebrew/bin/codex\n";
        assert_eq!(
            run_hook("done", INPUT, table),
            [(
                "ttyA".to_string(),
                "\u{1b}Ptmux;\u{1b}\u{1b}]777;tterm-agent;codex;done\u{7}\u{1b}\\".to_string()
            )]
        );
    }

    #[test]
    fn hook_under_the_shared_server_writes_to_every_codex_with_the_session() {
        let table = "ME 700 ?? zsh\n\
                     700 1 ?? /Users/me/.codex/packages/bin/codex\n\
                     500 400 TTY/ttyA codex\n\
                     400 300 TTY/ttyA -zsh\n\
                     300 1 ?? tmux: server\n\
                     600 1 TTY/ttyB /opt/homebrew/bin/codex\n\
                     610 1 TTY/ttyB codex\n\
                     800 1 TTY/ttyC zsh\n\
                     900 1 ? Codex (Service)\n";
        let report = "\u{1b}]777;tterm-agent;codex;processing;01a12490-4b68-76d0\u{7}";
        assert_eq!(
            run_hook("processing", INPUT, table),
            [
                (
                    "ttyA".to_string(),
                    format!("\u{1b}Ptmux;\u{1b}{report}\u{1b}\\")
                ),
                ("ttyB".to_string(), report.to_string()),
            ]
        );
        // Without a session to name, no terminal could tell it is its own.
        assert_eq!(run_hook("processing", "{}", table), []);
    }
}
