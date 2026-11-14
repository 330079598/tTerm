//! Codex: hooks in `hooks.json` under `$CODEX_HOME`.
//!
//! Codex starts hooks in a session of their own, without a controlling
//! terminal, and has no field to write a sequence for them. So each hook
//! finds the terminal itself. Run on its own, Codex is the first of the hook's
//! ancestors with one. By default it hands its sessions to a shared background
//! server, and the hook descends from that server instead: it then writes to
//! every terminal running Codex, with the session's id, for tTerm to keep the
//! report in the tab the session was submitted from. The walk stops at the
//! nearest Codex: the server may still be the child of the Codex that started
//! it, whose terminal is not the session's. Codex runs a hook only
//! once the user has trusted it in `/hooks`, and again after it changes.
//!
//! On Windows Codex runs `commandWindows` instead, through `cmd /C` and with
//! a hidden console of its own, where there is neither `ps` nor a terminal
//! device to write to. The hook runs tTerm itself there, which finds the
//! console the same way (see `console_report`).

use std::path::Path;

use crate::agent_integration::REPORT_ARG;
use crate::agent_integration::hooks::HookSet;

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
    command_windows: if cfg!(windows) {
        Some(windows_hook_command)
    } else {
        None
    },
};

/// tTerm's own executable run as the hook; none if it cannot be located or
/// named without quotes, which leaves Windows without reports as before.
///
/// Codex hands the command to `cmd /C` as one argument, quoted the way Rust
/// quotes arguments, so a `"` in it reaches cmd as `\"` and cmd looks for a
/// program named `\"C:\...`, failing every hook. The path goes in unquoted
/// instead: as it is when cmd reads it as one word, or else by its short name.
fn windows_hook_command(state: &str) -> Option<String> {
    report_command(&std::env::current_exe().ok()?, state)
}

/// The command running `exe` to report `state` for Codex.
fn report_command(exe: &Path, state: &str) -> Option<String> {
    let exe = cmd_word(exe)?;
    Some(format!("{exe} {REPORT_ARG} codex {state}"))
}

/// The path as one word cmd reads verbatim, if it or its short name is one.
fn cmd_word(path: &Path) -> Option<String> {
    let long = path.to_str()?;
    if is_cmd_word(long) {
        return Some(long.to_string());
    }
    short_path(path).filter(|short| is_cmd_word(short))
}

/// Whether cmd reads `text` as one word without changing it: no spaces,
/// separators (`,;=`), quotes, or characters it expands or escapes.
fn is_cmd_word(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || r"\:._-~+#$@[]{}'".contains(c))
}

/// The 8.3 name of an existing path, which has no spaces unless the volume
/// does not keep short names.
#[cfg(windows)]
fn short_path(path: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::GetShortPathNameW;
    use windows::core::PCWSTR;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let needed = unsafe { GetShortPathNameW(PCWSTR(wide.as_ptr()), None) };
    let mut buffer = vec![0u16; usize::try_from(needed).ok()?];
    let length = unsafe { GetShortPathNameW(PCWSTR(wide.as_ptr()), Some(&mut buffer)) };
    let length = usize::try_from(length).ok()?;
    if length == 0 || length >= buffer.len() {
        return None;
    }
    String::from_utf16(&buffer[..length]).ok()
}

#[cfg(not(windows))]
fn short_path(_path: &Path) -> Option<String> {
    None
}

/// A POSIX script under `sh -c`, so it runs the same whatever login shell
/// Codex starts it with. Its escapes are printf's: fish would read `\\` in
/// single quotes as one backslash, so the string terminator's is `\134`.
/// One `ps` lists the processes for awk, which prints each terminal to write
/// to, whether a tmux pane holds it, and whether the report goes there
/// directly (Codex's own terminal) or as a broadcast (every Codex
/// terminal). tmux passes the report on only wrapped, and with
/// `allow-passthrough` on.
pub(super) fn hook_command(state: &str) -> String {
    let script = [
        super::hook_input::script(),
        format!(
            r#"ps -A -o pid= -o ppid= -o tty= -o comm= 2>/dev/null | awk -v h=$PPID '{AWK}' | while read x t b; do r="codex;{state}"; if [ "$b" = 1 ]; then [ -n "$sid" ] || continue; r="$r;$sid;broadcast"; elif [ -n "$sid" ]; then r="$r;$sid;direct"; fi; [ -z "$sid" ] || r="$r;$cwd"; s="\033]777;tterm-agent;$r\007"; [ "$x" = 0 ] || s="\033Ptmux;\033$s\033\134"; {{ printf "%b" "$s" > "/dev/$t"; }} 2>/dev/null; done"#
        ),
    ]
    .join("; ");
    let escaped = script
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`");
    format!("sh -c \"{escaped}\"")
}

/// awk is single quoted inside the POSIX script. `?` or `??` stands for no terminal; tmux's server is `tmux` or
/// `tmux: server`. The nearest `codex` up the hook's ancestors runs it: with
/// a terminal it is Codex on its own, without one the shared server, even
/// when a Codex with a terminal started that server and is its parent.
const AWK: &str = concat!(
    r#"function has(p) { return t[p] != "" && t[p] !~ /^[?]/ } "#,
    r#"function emit(p, b,  q, x) { if (t[p] in s) return; s[t[p]] = 1; x = 0; for (q = p; q > 1; q = pp[q]) if (n[q] ~ /^tmux/) x = 1; print x, t[p], b } "#,
    r#"{ pp[$1] = $2; t[$1] = $3; c = $4; for (i = 5; i <= NF; i++) c = c " " $i; sub(/.*\//, "", c); n[$1] = c } "#,
    r#"END { for (p = h; p > 1 && !has(p) && n[p] != "codex"; p = pp[p]); if (p > 1 && has(p)) emit(p, 0); else for (p in n) if (n[p] == "codex" && has(p)) emit(p, 1) }"#,
);

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn script(command: &str) -> String {
        let escaped = command
            .strip_prefix("sh -c \"")
            .unwrap()
            .strip_suffix('"')
            .unwrap();
        let mut chars = escaped.chars();
        let mut script = String::new();
        while let Some(c) = chars.next() {
            script.push(if c == '\\' { chars.next().unwrap() } else { c });
        }
        script
    }

    #[test]
    fn hook_is_one_quoted_posix_script() {
        let command = hook_command("waiting");
        let script = script(&command);
        let status = std::process::Command::new("sh")
            .args(["-n", "-c", &script])
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
            .args(["-c", &script(&command)])
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
                "\u{1b}Ptmux;\u{1b}\u{1b}]777;tterm-agent;codex;done;01a12490-4b68-76d0;direct;\u{7}\u{1b}\\".to_string()
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
        let report = "\u{1b}]777;tterm-agent;codex;processing;01a12490-4b68-76d0;broadcast;\u{7}";
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

    #[test]
    fn hook_under_a_server_started_by_a_codex_writes_to_every_codex() {
        // Codex 0.166 leaves the server a child of the Codex that started it.
        let table = "ME 700 ?? zsh\n\
                     700 600 ?? /Users/me/.codex/packages/app-server-daemon/bin/codex\n\
                     600 550 TTY/ttyB codex\n\
                     550 1 TTY/ttyB -zsh\n\
                     500 450 TTY/ttyA codex\n\
                     450 1 TTY/ttyA -zsh\n";
        let report = "\u{1b}]777;tterm-agent;codex;done;01a12490-4b68-76d0;broadcast;\u{7}";
        assert_eq!(
            run_hook("done", INPUT, table),
            [
                ("ttyA".to_string(), report.to_string()),
                ("ttyB".to_string(), report.to_string()),
            ]
        );
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::super::json::Json;
    use super::*;

    #[test]
    fn hook_on_windows_runs_tterm_itself() {
        let preview = HOOKS.preview();
        let Some(Json::Object(hooks)) = preview.get("hooks") else {
            panic!()
        };
        let Some(Json::Array(groups)) = hooks.get("PermissionRequest") else {
            panic!()
        };
        let Json::Object(group) = &groups[0] else {
            panic!()
        };
        let Some(Json::Array(handlers)) = group.get("hooks") else {
            panic!()
        };
        let Json::Object(handler) = &handlers[0] else {
            panic!()
        };
        let exe = std::env::current_exe().unwrap();
        assert_eq!(
            handler.get("commandWindows").and_then(Json::as_str),
            Some(format!("{} --agent-report codex waiting", exe.display()).as_str())
        );
    }

    #[test]
    fn hook_path_never_needs_quotes() {
        assert!(is_cmd_word(r"C:\Users\张三\AppData\Local\tTerm\tTerm.exe"));
        assert!(!is_cmd_word(r"C:\Program Files\tTerm\tTerm.exe"));
        assert!(!is_cmd_word(r#""C:\tTerm\tTerm.exe""#));
        assert!(!is_cmd_word(r"C:\A&B\tTerm.exe"));
        assert!(!is_cmd_word(r"C:\%PATH%\tTerm.exe"));
    }

    /// Checking the command's text let a quoted path through that cmd could
    /// not run, so this runs it the way Codex does: as one argument to
    /// `cmd /C`, quoted by Rust's `Command`. A batch file stands in for
    /// tTerm and echoes the arguments it was given.
    #[test]
    fn cmd_runs_the_hook_as_codex_starts_it() {
        let id = std::process::id();
        for name in [format!("tterm-hook-{id}"), format!("tterm hook {id}")] {
            let dir = std::env::temp_dir().join(name);
            std::fs::create_dir_all(&dir).unwrap();
            let exe = dir.join("tTerm.cmd");
            std::fs::write(&exe, "@echo %*\r\n").unwrap();
            // Without short names a path with spaces gets no command at all.
            let output = report_command(&exe, "waiting").map(|command| {
                std::process::Command::new("cmd")
                    .arg("/C")
                    .arg(command)
                    .output()
                    .unwrap()
            });
            std::fs::remove_dir_all(&dir).unwrap();
            let Some(output) = output else {
                continue;
            };
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                "--agent-report codex waiting"
            );
        }
    }
}
