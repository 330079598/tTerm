use super::*;

#[tokio::test]
async fn proxy_join_emits_first_payload_once_and_resolves_all_after_claim() {
    let (broker, sink, owner) = setup();
    let first = begin(&broker, &owner, PromptKind::ProxyCommand, "hash");
    let mut req = request(PromptKind::ProxyCommand, "hash");
    req.event = event("different proxy name");
    let second = broker
        .begin(
            req,
            PromptOptions {
                owner,
                timeout: Duration::from_secs(120),
            },
        )
        .unwrap();
    assert_eq!(sink.events(), vec![event("hash")]);
    assert_eq!(broker.pending_proxy().len(), 1);
    let claim = broker.claim_proxy("hash").unwrap();
    assert_eq!(claim.request().event, event("hash"));
    assert!(broker.pending_proxy().is_empty());
    assert_eq!(
        broker.claim_proxy("hash").err().unwrap(),
        "The proxy command question has expired."
    );
    assert_eq!(broker.shared.lock().entries.len(), 1);
    claim.complete(true);
    for wait in [first, second] {
        assert_eq!(
            wait.wait().await.unwrap(),
            PromptOutcome::Answered(PromptReply::ProxyCommand(true))
        );
    }
    empty(&broker);
}

#[tokio::test(start_paused = true)]
async fn first_proxy_deadline_expires_entire_group_even_after_late_join() {
    let (broker, sink, owner) = setup();
    let first = tokio::spawn(begin(&broker, &owner, PromptKind::ProxyCommand, "hash").wait());
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(60)).await;
    let second = tokio::spawn(begin(&broker, &owner, PromptKind::ProxyCommand, "hash").wait());
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(60)).await;
    assert_eq!(first.await.unwrap().unwrap(), PromptOutcome::TimedOut);
    assert_eq!(
        second.await.unwrap().unwrap(),
        PromptOutcome::ReceiverClosed
    );
    assert_eq!(sink.events(), vec![event("hash"), event("expired")]);
    empty(&broker);
}

#[tokio::test]
async fn cancelling_one_proxy_owner_leaves_other_waiters() {
    let (broker, sink, first_owner) = setup();
    let second_owner = broker.register_owner(None).unwrap();
    let first = begin(&broker, &first_owner, PromptKind::ProxyCommand, "hash");
    let second = begin(&broker, &second_owner, PromptKind::ProxyCommand, "hash");
    broker.cancel_owner(&first_owner);
    assert_eq!(first.wait().await.unwrap(), PromptOutcome::Cancelled);
    assert_eq!(broker.pending_proxy().len(), 1);
    broker.claim_proxy("hash").unwrap().complete(true);
    assert_eq!(
        second.wait().await.unwrap(),
        PromptOutcome::Answered(PromptReply::ProxyCommand(true))
    );
    assert_eq!(sink.events().len(), 1);
    empty(&broker);
}

#[tokio::test]
async fn cancelled_claim_does_not_release_closed_waiter_or_remove_new_group() {
    let (broker, _, owner) = setup();
    let first = begin(&broker, &owner, PromptKind::ProxyCommand, "hash");
    let claim = broker.claim_proxy("hash").unwrap();
    broker.cancel_owner(&owner);
    let other = broker.register_owner(None).unwrap();
    let second = begin(&broker, &other, PromptKind::ProxyCommand, "hash");
    claim.complete(true);
    assert_eq!(first.wait().await.unwrap(), PromptOutcome::Cancelled);
    broker.claim_proxy("hash").unwrap().complete(false);
    assert_eq!(
        second.wait().await.unwrap(),
        PromptOutcome::Answered(PromptReply::ProxyCommand(false))
    );
    empty(&broker);
}

#[tokio::test]
async fn dropped_claim_rejects_group_and_old_wait_drop_preserves_new_generation() {
    let (broker, _, owner) = setup();
    let old = begin(&broker, &owner, PromptKind::ProxyCommand, "hash");
    let claim = broker.claim_proxy("hash").unwrap();
    let new = begin(&broker, &owner, PromptKind::ProxyCommand, "hash");
    drop(claim);
    drop(old);
    assert_eq!(broker.pending_proxy().len(), 1);
    broker.claim_proxy("hash").unwrap().complete(true);
    assert_eq!(
        new.wait().await.unwrap(),
        PromptOutcome::Answered(PromptReply::ProxyCommand(true))
    );
    empty(&broker);
}

#[tokio::test(start_paused = true)]
async fn claimed_waiter_still_times_out_without_expiring_new_group() {
    let (broker, sink, owner) = setup();
    let old = begin(&broker, &owner, PromptKind::ProxyCommand, "hash");
    let old_task = tokio::spawn(old.wait());
    tokio::task::yield_now().await;
    let claim = broker.claim_proxy("hash").unwrap();
    let new = begin(&broker, &owner, PromptKind::ProxyCommand, "hash");
    tokio::time::advance(Duration::from_secs(120)).await;
    assert_eq!(old_task.await.unwrap().unwrap(), PromptOutcome::TimedOut);
    claim.complete(true);
    assert_eq!(broker.pending_proxy().len(), 1);
    assert_eq!(sink.events(), vec![event("hash"), event("hash")]);
    drop(new);
    empty(&broker);
}

#[tokio::test]
async fn missing_or_wrong_namespace_cannot_claim_and_last_drop_cleans_up() {
    let (broker, _, owner) = setup();
    let auth = begin(&broker, &owner, PromptKind::Auth, "hash");
    assert!(broker.claim_proxy("hash").is_err());
    let proxy = begin(&broker, &owner, PromptKind::ProxyCommand, "hash");
    drop(proxy);
    assert!(broker.pending_proxy().is_empty());
    assert!(broker.claim_proxy("hash").is_err());
    broker.reply_auth("hash", None).unwrap();
    auth.wait().await.unwrap();
    empty(&broker);
}

#[tokio::test]
async fn claimed_group_retains_other_owner_until_approval_finishes() {
    let (broker, _, owner) = setup();
    let other = broker.register_owner(None).unwrap();
    let first = begin(&broker, &owner, PromptKind::ProxyCommand, "hash");
    let second = begin(&broker, &other, PromptKind::ProxyCommand, "hash");
    let claim = broker.claim_proxy("hash").unwrap();
    broker.cancel_owner(&owner);
    claim.complete(true);
    assert_eq!(first.wait().await.unwrap(), PromptOutcome::Cancelled);
    assert_eq!(
        second.wait().await.unwrap(),
        PromptOutcome::Answered(PromptReply::ProxyCommand(true))
    );
    empty(&broker);
}

#[tokio::test(start_paused = true)]
async fn old_claim_completion_and_timeout_do_not_touch_replacement_group() {
    let (broker, sink, owner) = setup();
    let old = begin(&broker, &owner, PromptKind::ProxyCommand, "hash");
    let task = tokio::spawn(old.wait());
    tokio::task::yield_now().await;
    let claim = broker.claim_proxy("hash").unwrap();
    claim.complete(false);
    let replacement = begin(&broker, &owner, PromptKind::ProxyCommand, "hash");
    tokio::time::advance(Duration::from_secs(121)).await;
    assert_eq!(
        task.await.unwrap().unwrap(),
        PromptOutcome::Answered(PromptReply::ProxyCommand(false))
    );
    assert_eq!(sink.events(), vec![event("hash"), event("hash")]);
    broker.claim_proxy("hash").unwrap().complete(true);
    replacement.wait().await.unwrap();
    empty(&broker);
}
