use serde_json::Value;

use super::{FISH_SHELL_INTEGRATION, POSIX_SHELL_INTEGRATION};

fn oracle() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/gpui-migration/reports/P1-11c-evidence/node-macos.json"
    )))
    .expect("the P1-11c oracle is valid JSON")
}

#[test]
fn reports_the_directory_with_percent_escaped_as_the_shell_expands_it() {
    // Both the zsh and the bash prompt function send it.
    assert_eq!(POSIX_SHELL_INTEGRATION.matches("file://%s").count(), 2);
    assert_eq!(
        POSIX_SHELL_INTEGRATION
            .matches(r#""${PWD//\%/%25}""#)
            .count(),
        2
    );
    assert!(FISH_SHELL_INTEGRATION.contains("(string replace --all '%' '%25' -- $PWD)"));
}

#[test]
fn names_the_host_so_a_shell_on_a_host_reached_with_ssh_is_told_apart() {
    assert_eq!(POSIX_SHELL_INTEGRATION.matches("file://%s%s").count(), 2);
    assert!(POSIX_SHELL_INTEGRATION.contains(r#""$?" "$HOST" "${PWD"#));
    assert!(POSIX_SHELL_INTEGRATION.contains(r#""$s" "$HOSTNAME" "${PWD"#));
    assert!(FISH_SHELL_INTEGRATION.contains("file://%s%s"));
    assert!(FISH_SHELL_INTEGRATION.contains("' $hostname (string replace"));
}

#[test]
fn matches_the_typescript_scripts_byte_for_byte() {
    let oracle = oracle();
    let scripts = &oracle["values"]["remoteShellIntegration.scripts"];

    assert_eq!(
        POSIX_SHELL_INTEGRATION,
        scripts["posix"].as_str().expect("posix script")
    );
    assert_eq!(
        FISH_SHELL_INTEGRATION,
        scripts["fish"].as_str().expect("fish script")
    );
}

#[test]
fn the_posix_script_keeps_its_escape_sequences_literal() {
    assert_eq!(POSIX_SHELL_INTEGRATION.matches(r"\033]133;").count(), 5);
    assert!(POSIX_SHELL_INTEGRATION.contains(r"\e]133;C\e\\"));
    assert_eq!(FISH_SHELL_INTEGRATION.matches(r"\e]133;").count(), 3);
    assert_eq!(FISH_SHELL_INTEGRATION.matches(r"\\'").count(), 4);
}
