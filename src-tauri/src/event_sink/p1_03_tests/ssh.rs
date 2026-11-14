use crate::event_sink::p1_03_tests::*;

#[test]
fn ssh_wire_contracts_match_pre_migration_fixtures() {
    check(
        AppEvent::Ssh(SshEvent::ConnectionProgress {
            tab_id: "tab-测试".into(),
            payload: SshConnectionProgressPayload::new("ready", "ok"),
        }),
        "ssh-connection-progress-tab-测试",
        EventTarget::Any,
        "progress_min",
    );
    let mut payload = SshConnectionProgressPayload::new("retrying", "retry")
        .hop(1, 2)
        .host("localhost", 22303)
        .username("alice")
        .network_latency(Some(3))
        .retry_attempt(2, 1.5)
        .retry_max(4)
        .reason("drop")
        .shell_integration(Some(ShellIntegrationStatus::Active {
            hostname: Some("test".into()),
            shell: Some("bash".into()),
        }));
    payload.auth = Some(AuthUsedPayload {
        method: "key",
        private_key_path: Some("/test/key".into()),
        reusable: true,
    });
    check(
        AppEvent::Ssh(SshEvent::ConnectionProgress {
            tab_id: "tab-测试".into(),
            payload,
        }),
        "ssh-connection-progress-tab-测试",
        EventTarget::Any,
        "progress_full",
    );
    for (key, distribution) in [
        ("os_none", None),
        (
            "os_full",
            Some(LinuxDistributionInfo {
                id: "ubuntu".into(),
                id_like: vec!["debian".into()],
                name: "Ubuntu".into(),
                pretty_name: "Ubuntu Test".into(),
                version_id: None,
            }),
        ),
    ] {
        check(
            AppEvent::Ssh(SshEvent::RemoteOs(RemoteOsPayload {
                tab_id: "tab-测试".into(),
                distribution,
            })),
            "ssh-remote-os",
            EventTarget::Default,
            key,
        );
    }
}

#[test]
fn shell_status_tags_and_omissions_remain_unchanged() {
    for (reason, expected) in [
        (UnavailableReason::NoReply, "noReply"),
        (UnavailableReason::WriteFailed, "writeFailed"),
        (UnavailableReason::Path, "path"),
        (UnavailableReason::Rejected, "rejected"),
        (UnavailableReason::TimedOut, "timedOut"),
        (UnavailableReason::Failed, "failed"),
    ] {
        assert_eq!(
            serde_json::to_value(ShellIntegrationStatus::Unavailable { reason }).unwrap(),
            serde_json::json!({"status":"unavailable","reason":{"code":expected}})
        );
    }
    assert_eq!(
        serde_json::to_value(ShellIntegrationStatus::Unavailable {
            reason: UnavailableReason::Shell {
                shell: "csh".into()
            }
        })
        .unwrap(),
        serde_json::json!({"status":"unavailable","reason":{"code":"shell","shell":"csh"}})
    );
    assert_eq!(
        serde_json::to_value(ShellIntegrationStatus::Active {
            hostname: None,
            shell: None
        })
        .unwrap(),
        serde_json::json!({"status":"active"})
    );
}
