use super::*;

#[test]
fn background_wire_contracts_keep_default_scope_and_null_units() {
    check(
        AppEvent::Background(BackgroundEvent::QuitRequested(QuitBlockers {
            tunnels: 2,
            unuploaded_files: vec!["test.conf".into()],
        })),
        "quit-requested",
        EventTarget::Default,
        "blockers",
    );
    for (event, name) in [
        (BackgroundEvent::CloseRequested, "close-requested"),
        (BackgroundEvent::WillQuit, "app-will-quit"),
        (BackgroundEvent::OpenTunnels, "tray-open-tunnels"),
        (BackgroundEvent::CheckUpdates, "tray-check-updates"),
    ] {
        check(
            AppEvent::Background(event),
            name,
            EventTarget::Default,
            "unit",
        );
    }
}
