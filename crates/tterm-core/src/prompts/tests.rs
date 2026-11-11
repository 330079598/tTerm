use super::*;
use crate::events::recording::RecordingEventSink;
use crate::events::tunnel::{TunnelEvent, TunnelStatus, TunnelTraffic};
use crate::events::{AppEvent, EventSink};
use std::sync::Mutex;

fn event(label: &str) -> AppEvent {
    // Prompt event domains arrive in C2/C3. An existing event exercises the
    // transport-independent broker without preempting another card's variants.
    AppEvent::Tunnel(TunnelEvent::Status(TunnelStatus::stopped(
        label,
        TunnelTraffic::default(),
    )))
}

fn request(kind: PromptKind, id: &str) -> PromptRequest {
    PromptRequest {
        id: PromptId(id.into()),
        kind,
        event: event(id),
        expired_event: Some(event("expired")),
    }
}

fn setup() -> (PromptBroker, Arc<RecordingEventSink>, PromptOwner) {
    let sink = Arc::new(RecordingEventSink::default());
    let broker = PromptBroker::new(sink.clone());
    let owner = broker.register_owner(None).unwrap();
    (broker, sink, owner)
}

fn begin(broker: &PromptBroker, owner: &PromptOwner, kind: PromptKind, id: &str) -> PromptWait {
    broker
        .begin(
            request(kind, id),
            PromptOptions {
                owner: owner.clone(),
                timeout: Duration::from_secs(120),
            },
        )
        .unwrap()
}

fn empty(broker: &PromptBroker) {
    let state = broker.shared.lock();
    assert!(state.active.is_empty());
    assert!(state.entries.is_empty());
}

#[tokio::test]
async fn replies_are_once_only_and_do_not_cross_namespaces() {
    let (broker, sink, owner) = setup();
    let host = begin(&broker, &owner, PromptKind::HostKey, "same");
    let auth = begin(&broker, &owner, PromptKind::Auth, "same");
    assert_eq!(
        broker.reply_auth("missing", None).unwrap_err(),
        "Authentication prompt expired"
    );
    broker.reply_host_key("same", true).unwrap();
    assert_eq!(
        broker.reply_host_key("same", false).unwrap_err(),
        "Host key prompt expired"
    );
    broker
        .reply_auth("same", Some(vec!["answer".into()]))
        .unwrap();
    assert_eq!(
        host.wait().await.unwrap(),
        PromptOutcome::Answered(PromptReply::HostKey(true))
    );
    assert_eq!(
        auth.wait().await.unwrap(),
        PromptOutcome::Answered(PromptReply::Auth(Some(vec!["answer".into()])))
    );
    assert_eq!(sink.events(), vec![event("same"), event("same")]);
    empty(&broker);
}

#[tokio::test]
async fn cancelling_one_question_keeps_owner_open() {
    let (broker, sink, owner) = setup();
    let wait = begin(&broker, &owner, PromptKind::Auth, "first");
    broker.reply_auth("first", None).unwrap();
    assert_eq!(
        wait.wait().await.unwrap(),
        PromptOutcome::Answered(PromptReply::Auth(None))
    );
    assert!(!owner.is_closed());
    drop(begin(&broker, &owner, PromptKind::Auth, "second"));
    assert_eq!(sink.events().len(), 3);
    empty(&broker);
}

#[tokio::test(start_paused = true)]
async fn timeout_removes_request_and_emits_only_the_supplied_expiration() {
    let (broker, sink, owner) = setup();
    let wait = begin(&broker, &owner, PromptKind::Auth, "expired");
    let task = tokio::spawn(wait.wait());
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(119)).await;
    assert!(!task.is_finished());
    tokio::time::advance(Duration::from_secs(1)).await;
    assert_eq!(task.await.unwrap().unwrap(), PromptOutcome::TimedOut);
    assert_eq!(sink.events(), vec![event("expired"), event("expired")]);
    assert!(broker.reply_auth("expired", None).is_err());
    empty(&broker);
    let mut req = request(PromptKind::HostKey, "host");
    req.expired_event = None;
    let host = broker
        .begin(
            req,
            PromptOptions {
                owner,
                timeout: Duration::ZERO,
            },
        )
        .unwrap();
    assert_eq!(host.wait().await.unwrap(), PromptOutcome::TimedOut);
    assert_eq!(sink.events().len(), 3);
}

#[tokio::test]
async fn owner_cancellation_expires_question_once_and_rejects_new_questions() {
    let (broker, sink, owner) = setup();
    let wait = begin(&broker, &owner, PromptKind::Auth, "cancel");
    broker.cancel_owner(&owner);
    broker.cancel_owner(&owner);
    assert!(owner.is_closed());
    assert_eq!(wait.wait().await.unwrap(), PromptOutcome::Cancelled);
    assert!(
        broker
            .begin(
                request(PromptKind::Auth, "later"),
                PromptOptions {
                    owner,
                    timeout: Duration::ZERO
                }
            )
            .is_err()
    );
    assert_eq!(sink.events(), vec![event("cancel"), event("expired")]);
    empty(&broker);
}

#[tokio::test]
async fn nonce_and_attempt_cancellation_are_precise() {
    let (broker, _, _) = setup();
    let session = SessionKey {
        tab_id: "tab".into(),
        session_nonce: 1,
    };
    let old = broker.register_owner(Some(session.clone())).unwrap();
    let attempt = broker.register_owner(Some(session.clone())).unwrap();
    let new_session = broker
        .register_owner(Some(SessionKey {
            session_nonce: 2,
            ..session.clone()
        }))
        .unwrap();
    let first = begin(&broker, &old, PromptKind::Auth, "old");
    let second = begin(&broker, &attempt, PromptKind::Auth, "attempt");
    let third = begin(&broker, &new_session, PromptKind::Auth, "new");
    broker.cancel_owner(&old);
    assert!(!attempt.is_closed());
    broker.cancel_session(&session);
    assert!(!new_session.is_closed());
    broker.reply_auth("new", None).unwrap();
    assert_eq!(first.wait().await.unwrap(), PromptOutcome::Cancelled);
    assert_eq!(second.wait().await.unwrap(), PromptOutcome::Cancelled);
    assert!(matches!(
        third.wait().await.unwrap(),
        PromptOutcome::Answered(_)
    ));
    empty(&broker);
}

#[tokio::test]
async fn dropping_or_aborting_wait_cleans_up_without_polling_deadline() {
    let (broker, _, owner) = setup();
    drop(begin(&broker, &owner, PromptKind::HostKey, "drop"));
    empty(&broker);
    let wait = begin(&broker, &owner, PromptKind::Auth, "abort");
    let task = tokio::spawn(wait.wait());
    tokio::task::yield_now().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    empty(&broker);
}

#[tokio::test]
async fn old_wait_drop_never_removes_reused_id() {
    let (broker, _, owner) = setup();
    let old = begin(&broker, &owner, PromptKind::Auth, "reused");
    broker.reply_auth("reused", None).unwrap();
    let new = begin(&broker, &owner, PromptKind::Auth, "reused");
    drop(old);
    broker.reply_auth("reused", Some(vec![])).unwrap();
    assert_eq!(
        new.wait().await.unwrap(),
        PromptOutcome::Answered(PromptReply::Auth(Some(vec![])))
    );
    empty(&broker);
}

#[tokio::test]
async fn shutdown_cancels_claimed_and_unclaimed_waiters() {
    let (broker, _, owner) = setup();
    let proxy = begin(&broker, &owner, PromptKind::ProxyCommand, "hash");
    let claim = broker.claim_proxy("hash").unwrap();
    let auth = begin(&broker, &owner, PromptKind::Auth, "auth");
    broker.shutdown();
    claim.complete(true);
    assert!(broker.register_owner(None).is_err());
    assert_eq!(proxy.wait().await.unwrap(), PromptOutcome::Cancelled);
    assert_eq!(auth.wait().await.unwrap(), PromptOutcome::Cancelled);
    empty(&broker);
}

#[test]
fn foreign_owner_empty_id_and_duplicate_are_rejected_without_emitting() {
    let (broker, sink, owner) = setup();
    let (other, _, foreign) = setup();
    assert!(
        broker
            .begin(
                request(PromptKind::Auth, "id"),
                PromptOptions {
                    owner: foreign.clone(),
                    timeout: Duration::ZERO
                }
            )
            .is_err()
    );
    broker.cancel_owner(&foreign);
    assert!(!foreign.is_closed());
    assert!(
        broker
            .begin(
                request(PromptKind::Auth, ""),
                PromptOptions {
                    owner: owner.clone(),
                    timeout: Duration::ZERO
                }
            )
            .is_err()
    );
    let first = begin(&broker, &owner, PromptKind::Auth, "id");
    assert!(
        broker
            .begin(
                request(PromptKind::Auth, "id"),
                PromptOptions {
                    owner,
                    timeout: Duration::ZERO
                }
            )
            .is_err()
    );
    assert_eq!(sink.events().len(), 1);
    drop(first);
    empty(&broker);
    empty(&other);
}

struct ReentrantSink(Mutex<Option<PromptBroker>>);
impl EventSink for ReentrantSink {
    fn emit(&self, _: AppEvent) -> Result<(), String> {
        self.0
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .reply_auth("instant", None)
    }
}

#[tokio::test]
async fn sink_can_reply_synchronously_without_locking_or_registration_race() {
    let sink = Arc::new(ReentrantSink(Mutex::new(None)));
    let broker = PromptBroker::new(sink.clone());
    *sink.0.lock().unwrap() = Some(broker.clone());
    let owner = broker.register_owner(None).unwrap();
    let wait = begin(&broker, &owner, PromptKind::Auth, "instant");
    assert_eq!(
        wait.wait().await.unwrap(),
        PromptOutcome::Answered(PromptReply::Auth(None))
    );
    *sink.0.lock().unwrap() = None;
    empty(&broker);
}

struct FailingSink;
impl EventSink for FailingSink {
    fn emit(&self, _: AppEvent) -> Result<(), String> {
        Err("transport gone".into())
    }
}

#[tokio::test(start_paused = true)]
async fn send_failure_keeps_legacy_wait_until_timeout() {
    let broker = PromptBroker::new(Arc::new(FailingSink));
    let owner = broker.register_owner(None).unwrap();
    assert_eq!(
        begin(&broker, &owner, PromptKind::Auth, "failure")
            .wait()
            .await
            .unwrap(),
        PromptOutcome::TimedOut
    );
    empty(&broker);
}

#[tokio::test]
async fn reply_and_cancel_have_exactly_one_winner() {
    let (broker, _, owner) = setup();
    let wait = begin(&broker, &owner, PromptKind::HostKey, "race");
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let reply = {
        let broker = broker.clone();
        let barrier = barrier.clone();
        tokio::spawn(async move {
            barrier.wait().await;
            broker.reply_host_key("race", true)
        })
    };
    barrier.wait().await;
    broker.cancel_owner(&owner);
    let replied = reply.await.unwrap().is_ok();
    let outcome = wait.wait().await.unwrap();
    assert_eq!(
        outcome,
        if replied {
            PromptOutcome::Answered(PromptReply::HostKey(true))
        } else {
            PromptOutcome::Cancelled
        }
    );
    empty(&broker);
}

mod cancellation;
mod proxy_tests;

#[tokio::test]
async fn owner_remains_cancellable_when_caller_drops_its_handle() {
    let (broker, _, _) = setup();
    let session = SessionKey {
        tab_id: "tab".into(),
        session_nonce: 9,
    };
    let owner = broker.register_owner(Some(session.clone())).unwrap();
    let wait = begin(&broker, &owner, PromptKind::Auth, "orphan");
    drop(owner);
    broker.cancel_session(&session);
    assert_eq!(wait.wait().await.unwrap(), PromptOutcome::Cancelled);
    empty(&broker);
}

#[tokio::test]
async fn false_host_reply_and_cancel_before_wait_are_not_timeouts() {
    let (broker, sink, owner) = setup();
    let host = begin(&broker, &owner, PromptKind::HostKey, "host");
    broker.reply_host_key("host", false).unwrap();
    assert_eq!(
        host.wait().await.unwrap(),
        PromptOutcome::Answered(PromptReply::HostKey(false))
    );
    let auth = begin(&broker, &owner, PromptKind::Auth, "auth");
    let task = tokio::spawn(auth.wait());
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    empty(&broker);
    assert_eq!(sink.events().len(), 3);
}

#[test]
fn closed_nonce_cannot_register_late_owner_but_explicit_new_lifetime_can() {
    let (broker, _, _) = setup();
    let session = SessionKey {
        tab_id: "late".into(),
        session_nonce: 42,
    };
    broker.cancel_session(&session);
    assert!(broker.register_owner(Some(session.clone())).is_err());
    let newer = SessionKey {
        session_nonce: 43,
        ..session.clone()
    };
    assert!(broker.register_owner(Some(newer)).is_ok());
    broker.open_session(&session);
    assert!(broker.register_owner(Some(session)).is_ok());
}

#[test]
fn removed_tab_retains_closed_generation_for_late_side_operations() {
    let (broker, _, _) = setup();
    let session = SessionKey {
        tab_id: "closed-tab".into(),
        session_nonce: 9,
    };
    broker.open_session(&session);
    broker.cancel_session(&session);
    let retained = broker.session_for_tab("closed-tab").unwrap();
    assert_eq!(retained, session);
    assert!(broker.register_owner(Some(retained)).is_err());
    assert!(broker.session_for_tab("tunnels").is_none());
}
