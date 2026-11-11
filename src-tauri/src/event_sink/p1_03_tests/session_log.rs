use super::*;

#[test]
fn log_errors_keep_null_tabs_and_absent_phase_distinct() {
    for (key, tab_id, phase) in [
        ("log_global", None, None),
        ("log_error", Some("tab-测试".into()), None),
        (
            "log_start",
            Some("tab-测试".into()),
            Some(LogErrorPhase::Start),
        ),
    ] {
        check(
            AppEvent::SessionLog(SessionLogEvent::Error(LogErrorPayload {
                tab_id,
                message: "disk".into(),
                phase,
            })),
            "terminal-log-error",
            EventTarget::Any,
            key,
        );
    }
    check(
        AppEvent::SessionLog(SessionLogEvent::Status(TerminalLogStatus {
            enabled: true,
            directory: "/test/logs".into(),
            active_sessions: 1,
            recording_tab_ids: vec!["tab-测试".into()],
            total_size_bytes: 42,
            last_error: None,
        })),
        "terminal-log-status",
        EventTarget::Any,
        "log_status",
    );
}
