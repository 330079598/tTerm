use super::*;

#[tokio::test]
async fn last_proxy_cancellation_expires_once_but_partial_cancellation_does_not() {
    let (broker, sink, first_owner) = setup();
    let other = broker.register_owner(None).unwrap();
    let first = begin(&broker, &first_owner, PromptKind::ProxyCommand, "hash");
    let second = begin(&broker, &other, PromptKind::ProxyCommand, "hash");
    broker.cancel_owner(&first_owner);
    assert_eq!(first.wait().await.unwrap(), PromptOutcome::Cancelled);
    assert_eq!(sink.events(), vec![event("hash")]);
    broker.cancel_owner(&other);
    broker.cancel_owner(&other);
    assert_eq!(second.wait().await.unwrap(), PromptOutcome::Cancelled);
    assert_eq!(sink.events(), vec![event("hash"), event("expired")]);
    assert!(broker.claim_proxy("hash").is_err());
    empty(&broker);
}

#[test]
fn last_waiter_drop_expires_all_unanswered_prompt_kinds() {
    for kind in [
        PromptKind::HostKey,
        PromptKind::Auth,
        PromptKind::ProxyCommand,
    ] {
        let (broker, sink, owner) = setup();
        drop(begin(&broker, &owner, kind, "id"));
        assert_eq!(sink.events(), vec![event("id"), event("expired")]);
        empty(&broker);
    }
}

#[tokio::test]
async fn cancelled_claim_does_not_expire_replacement_with_same_wire_id() {
    let (broker, sink, owner) = setup();
    let old = begin(&broker, &owner, PromptKind::ProxyCommand, "hash");
    let claim = broker.claim_proxy("hash").unwrap();
    let next_owner = broker.register_owner(None).unwrap();
    let next = begin(&broker, &next_owner, PromptKind::ProxyCommand, "hash");
    broker.cancel_owner(&owner);
    assert_eq!(old.wait().await.unwrap(), PromptOutcome::Cancelled);
    drop(claim);
    assert_eq!(sink.events(), vec![event("hash"), event("hash")]);
    broker.claim_proxy("hash").unwrap().complete(true);
    assert_eq!(
        next.wait().await.unwrap(),
        PromptOutcome::Answered(PromptReply::ProxyCommand(true))
    );
}
