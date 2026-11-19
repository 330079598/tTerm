use super::*;

use serde_json::{Value, json};

use crate::workspace::local_shells::connections::{
    local_shell_connection, starts_as_saved, starts_shell, with_current_local_shell,
    with_local_shell,
};
use crate::workspace::local_shells::saved::own_shell_choice;
use crate::workspace::local_shells::types::LocalShellChoice;
use crate::workspace::tab::{Tab, TabConnection, TabType, TerminalShellType};

fn own_choice() -> LocalShellChoice {
    LocalShellChoice {
        shell: "custom".to_string(),
        custom_path: Some(r"C:\nu\nu.exe".to_string()),
        shell_id: Some("nu".to_string()),
        ..LocalShellChoice::default()
    }
}

fn tab_with_nonce(nonce: Value) -> Tab {
    let mut tab = Tab::new("t", "Local", TabType::Terminal);
    tab.set_saved_field("sessionNonce", Some(nonce));
    tab
}

#[test]
fn carry_the_users_shell_id_so_a_restart_follows_its_edits() {
    let connection = local_shell_connection(&own_choice(), Some(r"C:\src"));
    assert_eq!(connection.saved_field("cwd"), Some(&json!(r"C:\src")));
    assert_eq!(
        connection.saved_field("terminalShell"),
        Some(&json!("custom"))
    );
    assert_eq!(
        connection.saved_field("terminalShellCustomPath"),
        Some(&json!(r"C:\nu\nu.exe"))
    );
    assert_eq!(connection.saved_field("terminalShellCustomArgs"), None);
    assert_eq!(
        connection.saved_field("terminalShellId"),
        Some(&json!("nu"))
    );
    let moved = shell("nu", "nu", r"D:\nu.exe", "");
    let current = with_current_local_shell(Some(&connection), &[moved]);
    assert_eq!(
        current
            .as_ref()
            .and_then(|current| current.saved_field("terminalShellCustomPath")),
        Some(&json!(r"D:\nu.exe"))
    );
    assert_eq!(
        with_current_local_shell(Some(&connection), &[]),
        Some(connection)
    );
    assert_eq!(with_current_local_shell(None, &[nu()]), None);
}

#[test]
fn leave_out_custom_fields_for_a_builtin_shell() {
    let builtin = LocalShellChoice {
        custom_path: Some("x".to_string()),
        shell_id: Some("y".to_string()),
        ..choice("pwsh")
    };
    let connection = local_shell_connection(&builtin, None);
    assert_eq!(connection.saved_field("cwd"), None);
    assert_eq!(
        connection.saved_field("terminalShell"),
        Some(&json!("pwsh"))
    );
    assert_eq!(connection.saved_field("terminalShellCustomPath"), None);
    assert_eq!(connection.saved_field("terminalShellCustomArgs"), None);
    assert_eq!(connection.saved_field("terminalShellId"), None);
    assert_eq!(connection.saved_field("terminalShellWslDistro"), None);
    assert_eq!(connection.terminal_shell, Some(TerminalShellType::Pwsh));
}

#[test]
fn saves_a_connection_in_the_frontend_key_order() {
    let custom = LocalShellChoice {
        custom_args: Some("--login -i".to_string()),
        ..own_choice()
    };
    let connection = local_shell_connection(&custom, Some(r"C:\src"));
    let saved = serde_json::to_value(&connection).expect("a connection serializes");
    let keys: Vec<&str> = saved
        .as_object()
        .expect("a connection is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "type",
            "cwd",
            "terminalShell",
            "terminalShellCustomPath",
            "terminalShellCustomArgs",
            "terminalShellId",
        ]
    );
    let auto = local_shell_connection(&LocalShellChoice::default(), None);
    let saved = serde_json::to_value(&auto).expect("a connection serializes");
    let keys: Vec<&str> = saved
        .as_object()
        .expect("a connection is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["type", "terminalShell"]);
}

#[test]
fn tells_whether_a_tab_starts_the_same_shell() {
    let own = own_choice();
    assert!(starts_shell(None, &LocalShellChoice::default()));
    let pwsh = local_shell_connection(&choice("pwsh"), None);
    assert!(!starts_shell(Some(&pwsh), &choice("cmd")));
    let own_connection = local_shell_connection(&own, None);
    assert!(starts_shell(Some(&own_connection), &own));
    let other = LocalShellChoice {
        shell_id: Some("other".to_string()),
        ..own.clone()
    };
    assert!(!starts_shell(Some(&own_connection), &other));
    let debian = LocalShellChoice {
        shell: "wsl".to_string(),
        wsl_distro: Some("Debian".to_string()),
        ..LocalShellChoice::default()
    };
    let debian_connection = local_shell_connection(&debian, None);
    assert!(starts_shell(Some(&debian_connection), &debian));
    assert!(!starts_shell(Some(&debian_connection), &choice("wsl")));
    let mut by_path = TabConnection::default();
    by_path.set_terminal_shell(Some(TerminalShellType::Custom));
    by_path.set_terminal_shell_custom_path(Some("/bin/zsh ".to_string()));
    let zsh = LocalShellChoice {
        shell: "custom".to_string(),
        custom_path: Some("/bin/zsh".to_string()),
        ..LocalShellChoice::default()
    };
    assert!(starts_shell(Some(&by_path), &zsh));
}

#[test]
fn keeps_the_tabs_directory_and_starts_a_new_session() {
    let mut tab = Tab::new("t", "Local", TabType::Terminal);
    let gone = LocalShellChoice {
        shell: "custom".to_string(),
        custom_path: Some("gone".to_string()),
        ..LocalShellChoice::default()
    };
    tab.set_connection(Some(local_shell_connection(&gone, Some("/src"))));
    tab.set_session_nonce(Some(3));
    let restarted = with_local_shell(&tab, &LocalShellChoice::default());
    assert_eq!(
        restarted.connection,
        Some(local_shell_connection(
            &LocalShellChoice::default(),
            Some("/src")
        ))
    );
    assert_eq!(restarted.session_nonce, Some(4));
}

#[test]
fn wraps_the_session_nonce_like_javascript() {
    let nonce = |value: Value| {
        with_local_shell(&tab_with_nonce(value), &LocalShellChoice::default()).session_nonce
    };
    assert_eq!(nonce(json!(3)), Some(4));
    assert_eq!(nonce(json!(4294967295_u64)), Some(0));
    assert_eq!(nonce(json!(4294967296_u64)), Some(1));
    assert_eq!(nonce(json!(4294967294.5)), Some(4294967295));
    assert_eq!(nonce(json!(-1)), Some(0));
    assert_eq!(nonce(json!(-3)), Some(4294967294));
    assert_eq!(nonce(json!(1.5)), Some(2));
    assert_eq!(nonce(json!(1_000_000_000_000_i64)), Some(3_567_587_329));
    let without = Tab::new("t", "Local", TabType::Terminal);
    assert_eq!(
        with_local_shell(&without, &LocalShellChoice::default()).session_nonce,
        Some(1)
    );
    assert_eq!(
        with_local_shell(&tab_with_nonce(Value::Null), &LocalShellChoice::default()).session_nonce,
        Some(1)
    );
}

#[test]
fn tells_whether_a_tab_starts_the_users_shell_as_it_is_saved_now() {
    let listed = msys();
    let connection = local_shell_connection(&own_shell_choice(&listed), None);
    assert!(starts_as_saved(Some(&connection), &listed));
    assert!(starts_as_saved(
        Some(&connection),
        &shell(
            "msys",
            "MSYS2",
            r"C:\msys64\usr\bin\bash.exe",
            " --login -i "
        )
    ));
    assert!(!starts_as_saved(
        Some(&connection),
        &shell("msys", "MSYS2", r"C:\msys64\usr\bin\bash.exe", "--login")
    ));
    assert!(!starts_as_saved(
        Some(&connection),
        &shell("msys", "MSYS2", r"D:\msys64\usr\bin\bash.exe", "--login -i")
    ));
}

#[test]
fn tells_an_unknown_shell_from_auto() {
    let unknown = choice("bogus");
    let connection = local_shell_connection(&unknown, None);
    assert!(starts_shell(Some(&connection), &unknown));
    assert!(!starts_shell(
        Some(&connection),
        &LocalShellChoice::default()
    ));
    assert!(!starts_shell(Some(&connection), &choice("cmd")));
    assert!(starts_shell(None, &LocalShellChoice::default()));
    let mut missing = TabConnection::default();
    assert!(starts_shell(Some(&missing), &LocalShellChoice::default()));
    missing.set_saved_field("terminalShell", Some(Value::Null));
    assert!(starts_shell(Some(&missing), &LocalShellChoice::default()));
    missing.set_saved_field("terminalShell", Some(json!(5)));
    assert!(!starts_shell(Some(&missing), &LocalShellChoice::default()));
}

#[test]
fn keeps_an_unknown_shell_id_through_a_connection_and_a_restart() {
    let unknown = choice("bogus");
    let connection = local_shell_connection(&unknown, Some("/src"));
    assert_eq!(
        connection.saved_field("terminalShell"),
        Some(&json!("bogus"))
    );
    assert_eq!(connection.terminal_shell, None);
    let mut tab = Tab::new("t", "Local", TabType::Terminal);
    tab.set_connection(Some(connection));
    tab.set_session_nonce(Some(3));
    let restarted = with_local_shell(&tab, &unknown);
    assert_eq!(
        restarted
            .connection
            .as_ref()
            .and_then(|connection| connection.saved_field("terminalShell")),
        Some(&json!("bogus"))
    );
    assert_eq!(
        restarted
            .connection
            .as_ref()
            .and_then(|connection| connection.saved_field("cwd")),
        Some(&json!("/src"))
    );
    assert_eq!(restarted.session_nonce, Some(4));
}

#[test]
fn keeps_the_other_fields_of_a_restarted_tab() {
    let mut tab = Tab::new("t", "Bogus", TabType::Terminal);
    tab.set_is_active(true);
    tab.set_saved_field("connectionHeaderPinned", Some(json!(true)));
    tab.set_saved_field("unknownField", Some(json!({"kept": 1})));
    tab.set_connection(Some(local_shell_connection(&choice("bogus"), Some("/src"))));
    let restarted = with_local_shell(&tab, &LocalShellChoice::default());
    assert_eq!(restarted.id, "t");
    assert_eq!(restarted.title, "Bogus");
    assert!(restarted.is_active);
    assert_eq!(
        restarted.saved_field("connectionHeaderPinned"),
        Some(&json!(true))
    );
    assert_eq!(
        restarted.saved_field("unknownField"),
        Some(&json!({"kept": 1}))
    );
    assert_eq!(
        restarted
            .connection
            .as_ref()
            .and_then(|connection| connection.saved_field("terminalShell")),
        Some(&json!("auto"))
    );
}
