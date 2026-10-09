//! Claude Code: hooks in `settings.json` under its config directory.
//!
//! Hooks run without a controlling terminal, so they cannot write to the
//! terminal themselves; each prints `{"terminalSequence": …}` and Claude Code
//! writes the sequence for it. Only synchronous hooks get theirs written,
//! and only while Claude Code's interface is up (not from `SessionStart`).

use super::hooks::HookSet;

/// The hook events tTerm listens to and the state each one reports.
/// `PermissionRequest` fires the moment Claude asks; the `Notification` hook's
/// `permission_prompt` waits about six seconds first. tTerm itself waits a
/// moment before announcing a wait, as auto mode may approve the request.
/// `SessionEnd`'s report is often lost as Claude Code exits first; tTerm
/// also forgets the agent at the shell's next prompt or the session's end.
pub(super) const HOOKS: HookSet = HookSet {
    file: "settings.json",
    events: &[
        ("UserPromptSubmit", "processing"),
        ("PostToolUse", "processing"),
        ("PostToolUseFailure", "processing"),
        ("ElicitationResult", "processing"),
        ("PermissionRequest", "waiting"),
        ("Elicitation", "waiting"),
        ("Stop", "done"),
        ("StopFailure", "error"),
        ("SessionEnd", "ended"),
    ],
    command: hook_command,
};

/// `printf '%s'` so no shell interprets the `\u` escapes, which have to reach
/// Claude Code's JSON parser as they are.
fn hook_command(state: &str) -> String {
    format!(r#"printf '%s' '{{"terminalSequence":"\u001b]777;tterm-agent;claude;{state}\u0007"}}'"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn hook_prints_the_json_claude_code_expects() {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(hook_command("waiting"))
            .output()
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            value["terminalSequence"],
            "\u{1b}]777;tterm-agent;claude;waiting\u{7}"
        );
    }
}
