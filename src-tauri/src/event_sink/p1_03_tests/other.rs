use super::*;

#[test]
fn cwd_notification_and_launch_contracts_match_legacy() {
    check(
        AppEvent::Terminal(TerminalEvent::Cwd {
            tab_id: "tab-测试".into(),
            payload: CwdPayload {
                session_nonce: 7,
                cwd: "/test/目录".into(),
            },
        }),
        "pty-cwd-tab-测试",
        EventTarget::Any,
        "cwd",
    );
    for (key, tab_id) in [
        ("activated", Some("tab-测试".into())),
        ("activated_none", None),
    ] {
        check(
            AppEvent::Notification(NotificationEvent::Activated(ActivatedPayload { tab_id })),
            "notification-activated",
            EventTarget::Default,
            key,
        );
    }
    check(
        AppEvent::LaunchDir(LaunchDirEvent::Queued),
        "launch-dirs",
        EventTarget::Default,
        "unit",
    );
}
