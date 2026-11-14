use crate::event_sink::p1_03_tests::*;

#[test]
fn zmodem_and_serial_keep_the_same_transfer_wire_protocol() {
    check(
        AppEvent::Zmodem(ZmodemEvent::SendRequested {
            tab_id: "tab-测试".into(),
        }),
        "zmodem-send-requested-tab-测试",
        EventTarget::Any,
        "unit",
    );
    for (event, kind, key) in transfers() {
        let name = format!("zmodem-transfer-{kind}-tab-测试");
        check(
            AppEvent::Zmodem(ZmodemEvent::Transfer {
                tab_id: "tab-测试".into(),
                event: event.clone(),
            }),
            &name,
            EventTarget::Any,
            key,
        );
        check(
            AppEvent::Serial(SerialEvent::Transfer {
                tab_id: "tab-测试".into(),
                event,
            }),
            &name,
            EventTarget::Any,
            key,
        );
    }
}

fn transfers() -> Vec<(TransferEvent, &'static str, &'static str)> {
    vec![
        (
            TransferEvent::Start(TransferStartPayload {
                transfer_id: "transfer-1".into(),
                direction: "upload",
                file_name: "测试.bin".into(),
                file_size: 42,
                local_path: "/test/file".into(),
                size_unknown: false,
            }),
            "start",
            "start",
        ),
        (
            TransferEvent::Start(TransferStartPayload {
                transfer_id: "transfer-1".into(),
                direction: "download",
                file_name: "测试.bin".into(),
                file_size: 0,
                local_path: "/test/file".into(),
                size_unknown: true,
            }),
            "start",
            "start_unknown",
        ),
        (
            TransferEvent::Progress(TransferProgressPayload {
                transfer_id: "transfer-1".into(),
                transferred: 42,
                speed: 21,
            }),
            "progress",
            "progress",
        ),
        (
            TransferEvent::Complete(TransferCompletePayload {
                transfer_id: "transfer-1".into(),
                success: true,
                cancelled: false,
                error: None,
            }),
            "complete",
            "complete",
        ),
        (
            TransferEvent::Complete(TransferCompletePayload {
                transfer_id: "transfer-1".into(),
                success: false,
                cancelled: true,
                error: Some("cancel".into()),
            }),
            "complete",
            "cancelled",
        ),
    ]
}
