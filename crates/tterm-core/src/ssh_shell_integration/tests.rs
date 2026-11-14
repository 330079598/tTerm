use serde_json::Value;

use crate::events::ssh::UnavailableReason;

use super::{
    ShellIntegrationChoice, announces_shell_integration_fallback, shell_integration_choice,
    shell_integration_payload, ssh_shell_integration_enabled,
};

fn oracle() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/gpui-migration/reports/P1-11c-evidence/node-macos.json"
    )))
    .expect("the P1-11c oracle is valid JSON")
}

#[test]
fn round_trips_through_the_stored_value() {
    for value in [None, Some(true), Some(false)] {
        assert_eq!(
            shell_integration_payload(shell_integration_choice(value)),
            value
        );
    }
}

#[test]
fn follows_the_setting_unless_the_connection_chose() {
    assert!(ssh_shell_integration_enabled(None, true));
    assert!(!ssh_shell_integration_enabled(None, false));
    assert!(!ssh_shell_integration_enabled(Some(false), true));
    assert!(ssh_shell_integration_enabled(Some(true), false));
}

#[test]
fn stays_quiet_about_the_hosts_own_answers_unless_the_connection_turned_it_on() {
    let reasons = [
        UnavailableReason::Shell {
            shell: "tcsh".to_string(),
        },
        UnavailableReason::NoReply,
        UnavailableReason::WriteFailed,
        UnavailableReason::Path,
    ];
    for reason in &reasons {
        assert!(
            !announces_shell_integration_fallback(None, reason),
            "{reason:?}"
        );
        assert!(
            announces_shell_integration_fallback(Some(true), reason),
            "{reason:?}"
        );
    }
}

#[test]
fn tells_about_failures_a_reconnect_may_not_meet_again() {
    for reason in [
        UnavailableReason::Rejected,
        UnavailableReason::TimedOut,
        UnavailableReason::Failed,
    ] {
        assert!(announces_shell_integration_fallback(None, &reason));
    }
}

#[test]
fn oracle_matches_the_choice_and_payload_forms() {
    let oracle = oracle();
    let values = &oracle["values"]["sshShellIntegration.choice"];

    assert_eq!(
        shell_integration_choice(None).as_str(),
        values["missing"].as_str().expect("missing")
    );
    assert_eq!(
        shell_integration_choice(Some(true)).as_str(),
        values["on"].as_str().expect("on")
    );
    assert_eq!(
        shell_integration_choice(Some(false)).as_str(),
        values["off"].as_str().expect("off")
    );

    let payload = values["payload"].as_array().expect("payload");
    assert_eq!(
        shell_integration_payload(ShellIntegrationChoice::Default),
        None
    );
    assert_eq!(
        shell_integration_payload(ShellIntegrationChoice::On),
        payload[1].as_bool()
    );
    assert_eq!(
        shell_integration_payload(ShellIntegrationChoice::Off),
        payload[2].as_bool()
    );
    assert!(payload[0].is_null(), "a default choice has no payload");
}
