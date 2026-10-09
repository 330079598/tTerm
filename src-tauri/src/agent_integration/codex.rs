//! Codex: hooks in `hooks.json` under `$CODEX_HOME`.
//!
//! Codex starts hooks in a session of their own, without a controlling
//! terminal, and has no field to write a sequence for them. So each hook
//! finds the terminal itself: the first of its ancestors with one, which is
//! Codex. Codex runs a hook only once the user has trusted it in `/hooks`,
//! and again after it changes.

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
/// tmux passes the report on only wrapped, and with `allow-passthrough` on.
fn hook_command(state: &str) -> String {
    let script = [
        "p=$PPID; t=",
        r#"while [ "${p:-1}" -gt 1 ]; do t=$(ps -o tty= -p "$p" 2>/dev/null | tr -d " "); case "$t" in ""|"?"|"??") t=; p=$(ps -o ppid= -p "$p" 2>/dev/null | tr -d " ");; *) break;; esac; done"#,
        &format!(r#"s="\033]777;tterm-agent;codex;{state}\007""#),
        r#"[ -z "$TMUX" ] || s="\033Ptmux;\033$s\033\134""#,
        r#"[ -z "$t" ] || printf "$s" > "/dev/$t""#,
    ]
    .join("; ");
    format!("sh -c '{script}'")
}

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

    #[test]
    fn hook_formats_the_report_and_its_tmux_wrapping() {
        let command = hook_command("done");
        // The report as printf writes it, minus the search for a terminal.
        let format = |tmux: &str| {
            let script = script(&command);
            let start = script.find("s=").unwrap();
            let end = script.find(r#"[ -z "$t" ]"#).unwrap();
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!(r#"{} printf "$s""#, &script[start..end]))
                .env("TMUX", tmux)
                .output()
                .unwrap();
            String::from_utf8(output.stdout).unwrap()
        };
        assert_eq!(format(""), "\u{1b}]777;tterm-agent;codex;done\u{7}");
        assert_eq!(
            format("/tmp/tmux-501/default,1,0"),
            "\u{1b}Ptmux;\u{1b}\u{1b}]777;tterm-agent;codex;done\u{7}\u{1b}\\"
        );
    }
}
