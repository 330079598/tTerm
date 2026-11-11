use super::*;

#[tokio::test]
async fn many_closed_tabs_and_nonces_leave_no_session_history_or_accepted_late_reply() {
    let (broker, _, _) = setup();
    for nonce in 0..1000 {
        for tab in ["reused".to_string(), format!("tab-{nonce}")] {
            let session = SessionKey {
                tab_id: tab,
                session_nonce: nonce,
            };
            broker.open_session(&session);
            let owner = broker.register_owner(Some(session.clone())).unwrap();
            let wait = begin(&broker, &owner, PromptKind::HostKey, "host");
            broker.cancel_session(&session);
            assert_eq!(wait.wait().await.unwrap(), PromptOutcome::Cancelled);
            assert!(broker.reply_host_key("host", true).is_err());
            assert!(broker.register_owner(Some(session)).is_err());
            assert!(broker.shared.lock().sessions.is_empty());
            empty(&broker);
        }
    }
}

#[test]
fn stale_close_cannot_remove_new_session_and_old_nonce_cannot_register() {
    let (broker, _, _) = setup();
    let old = SessionKey {
        tab_id: "tab".into(),
        session_nonce: 1,
    };
    let new = SessionKey {
        session_nonce: 2,
        ..old.clone()
    };
    broker.open_session(&old);
    broker.open_session(&new);
    broker.cancel_session(&old);
    assert!(broker.register_owner(Some(old)).is_err());
    assert!(broker.register_owner(Some(new.clone())).is_ok());
    assert_eq!(broker.shared.lock().sessions.len(), 1);
    broker.cancel_session(&new);
    assert!(broker.shared.lock().sessions.is_empty());
}
