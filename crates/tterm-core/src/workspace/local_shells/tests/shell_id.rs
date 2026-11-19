use crate::workspace::local_shells::shell_type_from_id;
use crate::workspace::tab::TerminalShellType;

#[test]
fn reads_each_known_shell_id_as_its_kind() {
    for shell in [
        TerminalShellType::Auto,
        TerminalShellType::Cmd,
        TerminalShellType::Powershell,
        TerminalShellType::Pwsh,
        TerminalShellType::Wsl,
        TerminalShellType::GitBash,
        TerminalShellType::Custom,
    ] {
        let saved = serde_json::to_value(shell).expect("a shell kind serializes");
        let id = saved.as_str().expect("a shell kind saves as its id");
        assert_eq!(shell_type_from_id(id), Some(shell));
    }
    assert_eq!(shell_type_from_id("bogus"), None);
    assert_eq!(shell_type_from_id(""), None);
}
