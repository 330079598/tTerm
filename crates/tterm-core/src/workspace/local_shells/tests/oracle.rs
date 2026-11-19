use super::*;

use crate::workspace::local_shells::connections::with_local_shell;
use crate::workspace::local_shells::default_shell::shell_choice_key;
use crate::workspace::local_shells::saved::own_shell_choice;
use crate::workspace::local_shells::shell_label::{is_detected_custom_shell, terminal_shell_label};
use crate::workspace::local_shells::types::{LocalShellChoice, ShellConfig};
use crate::workspace::local_shells::{
    AUTO_SHELL_KEY, custom_shell_key, detected_shell_key, program_name, wsl_key,
};
use crate::workspace::tab::{Tab, TabType};

#[test]
fn oracle_matches_the_ported_shell_labels() {
    let values = oracle()["values"].clone();
    let labels = &values["terminalShells.labels"];
    let label = |shell: &str, source: &str, distro: Option<&str>| {
        let mut profile = profile(shell, &format!("{shell} label"), source);
        profile.wsl_distro = distro.map(str::to_string);
        Value::String(terminal_shell_label(&profile, labelled))
    };
    assert_eq!(label("auto", "src", None), labels["auto"]);
    assert_eq!(label("cmd", "src", None), labels["cmd"]);
    assert_eq!(label("powershell", "src", None), labels["powershell"]);
    assert_eq!(label("pwsh", "src", None), labels["pwsh"]);
    assert_eq!(label("wsl", "src", None), labels["wsl"]);
    assert_eq!(label("git-bash", "src", None), labels["gitBash"]);
    assert_eq!(label("custom", "manual path", None), labels["custom"]);
    assert_eq!(label("custom", "/bin/zsh", None), labels["detectedCustom"]);
    assert_eq!(label("wsl", "src", Some("Ubuntu")), labels["wslDistro"]);

    let detected = &values["terminalShells.isDetectedCustomShell"];
    let found = |shell: &str, source: &str| {
        Value::Bool(is_detected_custom_shell(&profile(shell, "label", source)))
    };
    assert_eq!(found("custom", "manual path"), detected["manualPath"]);
    assert_eq!(found("custom", "$SHELL"), detected["shellVariable"]);
    assert_eq!(found("custom", "/bin/zsh"), detected["detected"]);
    assert_eq!(found("pwsh", "x"), detected["builtin"]);
}

#[test]
fn oracle_matches_the_ported_shell_keys_and_names() {
    let values = oracle()["values"].clone();
    let keys = &values["localShells.keys"];
    assert_eq!(
        Value::String(AUTO_SHELL_KEY.to_string()),
        keys["AUTO_SHELL_KEY"]
    );
    assert_eq!(Value::String(custom_shell_key("nu")), keys["custom"]);
    assert_eq!(Value::String(wsl_key("Ubuntu")), keys["wsl"]);

    let detected = &values["localShells.detectedShellKey"];
    assert_eq!(
        Value::String(detected_shell_key(&profile("pwsh", "", "x"))),
        detected["pwsh"]
    );
    assert_eq!(
        Value::String(detected_shell_key(&profile("custom", "", "/bin/zsh"))),
        detected["custom"]
    );
    assert_eq!(
        Value::String(detected_shell_key(&wsl_profile("", "wsl.exe", "Ubuntu"))),
        detected["wsl"]
    );

    let names = &values["localShells.programName"];
    assert_eq!(
        Value::String(program_name(r#""C:\Program Files\nu\nu.exe""#)),
        names["quotedWindows"]
    );
    assert_eq!(
        Value::String(program_name("/opt/homebrew/bin/fish")),
        names["unix"]
    );
    assert_eq!(Value::String(program_name("pwsh")), names["bare"]);
    assert_eq!(Value::String(program_name(".hidden")), names["dotted"]);

    let extra = &values["localShells.extra"];
    assert_eq!(
        Value::String(program_name("nu .exe")),
        extra["programNameTrailingSpace"]
    );
    assert_eq!(
        Value::String(program_name("\"\"nu.exe\"\"")),
        extra["programNameTwoQuotes"]
    );
    let mut tab = Tab::new("t", "Local", TabType::Terminal);
    tab.set_session_nonce(Some(u32::MAX));
    let restarted = with_local_shell(&tab, &LocalShellChoice::default());
    assert_eq!(
        Value::from(restarted.session_nonce.unwrap_or_default()),
        extra["sessionNonceWrap"]
    );
}

#[test]
fn oracle_matches_the_ported_choice_keys() {
    let values = oracle()["values"].clone();
    let expected = &values["localShells.shellChoiceKey"];
    let shells = vec![nu()];
    let config = ShellConfig {
        local_shells: &shells,
        ..ShellConfig::default()
    };
    assert_eq!(
        Value::String(shell_choice_key(&own_shell_choice(&nu()), &config)),
        expected["nu"]
    );
    let debian = LocalShellChoice {
        shell: "wsl".to_string(),
        wsl_distro: Some("Debian".to_string()),
        ..LocalShellChoice::default()
    };
    assert_eq!(
        Value::String(shell_choice_key(&debian, &ShellConfig::default())),
        expected["debian"]
    );
}
