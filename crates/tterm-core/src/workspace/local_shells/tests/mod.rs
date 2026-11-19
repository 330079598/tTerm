//! Specification tests from `src/lib/localShells.test.ts`, the label rules of
//! `src/lib/terminalShells.ts`, and the oracle those two produced.

mod connections;
mod default_shell;
mod entries;
mod oracle;
mod program_name;
mod saved;
mod shell_id;
mod shell_label;

use serde_json::Value;

use crate::config::local_shells::LocalShell;
use crate::terminal::pty::TerminalShellProfile;

use super::types::LocalShellChoice;

/// The frontend's `t`, which shows the label it is given.
fn translate(_key: &str, fallback: &str) -> String {
    fallback.to_string()
}

/// The frontend's `t`, keeping the key so a test sees which one was asked for.
fn labelled(key: &str, fallback: &str) -> String {
    format!("{key}|{fallback}")
}

fn shell(id: &str, name: &str, path: &str, args: &str) -> LocalShell {
    LocalShell {
        id: id.to_string(),
        name: name.to_string(),
        path: path.to_string(),
        args: args.to_string(),
    }
}

fn nu() -> LocalShell {
    shell("nu", "nu", r"C:\nu\nu.exe", "")
}

fn msys() -> LocalShell {
    shell("msys", "MSYS2", r"C:\msys64\usr\bin\bash.exe", "--login -i")
}

fn profile(shell: &str, label: &str, source: &str) -> TerminalShellProfile {
    TerminalShellProfile {
        shell: shell.to_string(),
        label: label.to_string(),
        source: source.to_string(),
        ..TerminalShellProfile::default()
    }
}

fn wsl_profile(label: &str, source: &str, distro: &str) -> TerminalShellProfile {
    TerminalShellProfile {
        wsl_distro: Some(distro.to_string()),
        ..profile("wsl", label, source)
    }
}

fn windows_shells() -> Vec<TerminalShellProfile> {
    vec![
        profile("auto", "Auto (recommended)", "tTerm default"),
        profile(
            "cmd",
            "Command Prompt (cmd)",
            r"C:\Windows\System32\cmd.exe",
        ),
        profile("pwsh", "PowerShell 7 (pwsh)", r"C:\PowerShell\7\pwsh.exe"),
        profile("custom", "Custom executable", "manual path"),
    ]
}

fn unix_shells() -> Vec<TerminalShellProfile> {
    vec![
        profile("auto", "Auto (recommended)", "tTerm default"),
        profile("custom", "zsh", "/bin/zsh"),
        profile("custom", "fish", "/opt/homebrew/bin/fish"),
        profile("custom", "Custom executable", "manual path"),
    ]
}

/// A choice of `shell` alone, as the frontend writes `{ shell }`.
fn choice(shell: &str) -> LocalShellChoice {
    LocalShellChoice {
        shell: shell.to_string(),
        ..LocalShellChoice::default()
    }
}

/// The expectations generated from the real TypeScript modules by
/// `scripts/migration/P1-11c-oracle.mjs`.
fn oracle() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/gpui-migration/reports/P1-11c-evidence/node-macos.json"
    )))
    .expect("the P1-11c oracle is valid JSON")
}
