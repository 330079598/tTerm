use super::*;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

struct Fake {
    closed: Arc<AtomicBool>,
    closes: Arc<AtomicUsize>,
}

impl HubConnection for Fake {
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    async fn close(&self) {
        self.closes.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Clone, Default)]
struct Probe {
    dials: Arc<AtomicUsize>,
    closes: Arc<AtomicUsize>,
}

impl Probe {
    fn fake(&self) -> (Fake, Arc<AtomicBool>) {
        let closed = Arc::new(AtomicBool::new(false));
        (
            Fake {
                closed: closed.clone(),
                closes: self.closes.clone(),
            },
            closed,
        )
    }

    /// A dial that takes a moment, like a real handshake.
    async fn dial(&self) -> Result<Fake, SshConnectError> {
        self.dials.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(30)).await;
        Ok(self.fake().0)
    }
}

fn key(id: &str, credentials: u64) -> SessionKey {
    SessionKey {
        id: id.to_string(),
        credentials,
    }
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(20)).await;
}

#[tokio::test]
async fn tunnels_asking_together_share_one_dial_and_one_connection() {
    let hub = SessionHub::<Fake>::default();
    let probe = Probe::default();
    let k = key("host", 1);

    let (a, b, c) = tokio::join!(
        hub.acquire(&k, |_| probe.dial()),
        hub.acquire(&k, |_| probe.dial()),
        hub.acquire(&k, |_| probe.dial()),
    );
    let (a, b, c) = (a.unwrap(), b.unwrap(), c.unwrap());

    assert_eq!(probe.dials.load(Ordering::SeqCst), 1);
    assert!(Arc::ptr_eq(&a.shared, &b.shared) && Arc::ptr_eq(&b.shared, &c.shared));
}

#[tokio::test]
async fn only_the_last_release_closes_the_connection() {
    let hub = SessionHub::<Fake>::default();
    let probe = Probe::default();
    let k = key("host", 1);
    let a = hub.acquire(&k, |_| probe.dial()).await.unwrap();
    let b = hub.acquire(&k, |_| probe.dial()).await.unwrap();

    drop(a);
    settle().await;
    assert_eq!(probe.closes.load(Ordering::SeqCst), 0);

    drop(b);
    settle().await;
    assert_eq!(probe.closes.load(Ordering::SeqCst), 1);

    // With nobody left, the next tunnel dials afresh.
    let _c = hub.acquire(&k, |_| probe.dial()).await.unwrap();
    assert_eq!(probe.dials.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn different_hosts_get_different_connections() {
    let hub = SessionHub::<Fake>::default();
    let probe = Probe::default();
    let a = hub.acquire(&key("one", 1), |_| probe.dial()).await.unwrap();
    let b = hub.acquire(&key("two", 1), |_| probe.dial()).await.unwrap();
    assert!(!Arc::ptr_eq(&a.shared, &b.shared));
    assert_eq!(probe.dials.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_dead_connection_is_replaced_and_the_old_lease_does_not_disturb_the_new_one() {
    let hub = SessionHub::<Fake>::default();
    let probe = Probe::default();
    let k = key("host", 1);

    let (fake, closed) = probe.fake();
    let old = hub.acquire(&k, |_| async { Ok(fake) }).await.unwrap();
    closed.store(true, Ordering::SeqCst);

    let fresh = hub.acquire(&k, |_| probe.dial()).await.unwrap();
    assert!(!Arc::ptr_eq(&old.shared, &fresh.shared));

    // The tunnel that noticed the drop lets go of the dead one…
    drop(old);
    settle().await;
    // …and the new connection is still the one others get.
    let other = hub.acquire(&k, |_| probe.dial()).await.unwrap();
    assert!(Arc::ptr_eq(&other.shared, &fresh.shared));
    assert_eq!(probe.dials.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_failed_dial_is_shared_only_with_tunnels_that_waited_for_it() {
    let hub = SessionHub::<Fake>::default();
    let k = key("host", 7);
    let attempts = Arc::new(AtomicUsize::new(0));
    let failing = |attempts: Arc<AtomicUsize>| async move {
        attempts.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(30)).await;
        Err::<Fake, _>(SshConnectError::Permanent("bad password".into()))
    };

    let (a, b) = tokio::join!(
        hub.acquire(&k, |_| failing(attempts.clone())),
        hub.acquire(&k, |_| failing(attempts.clone())),
    );
    assert!(a.is_err() && b.is_err());
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        1,
        "one attempt served both"
    );

    // A tunnel that asks later, or with other secrets, tries for itself.
    assert!(hub
        .acquire(&k, |_| failing(attempts.clone()))
        .await
        .is_err());
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_failure_is_not_inherited_by_tunnels_using_other_secrets() {
    let hub = SessionHub::<Fake>::default();
    let probe = Probe::default();
    let bad = key("host", 1);
    let good = key("host", 2);

    let (failed, ok) = tokio::join!(
        hub.acquire(&bad, |_| async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            Err::<Fake, _>(SshConnectError::Permanent("bad password".into()))
        }),
        async {
            // Queue behind the failing dial, then try with the right secrets.
            tokio::time::sleep(Duration::from_millis(5)).await;
            hub.acquire(&good, |_| probe.dial()).await
        },
    );
    assert!(failed.is_err());
    assert!(ok.is_ok(), "good credentials must get their own attempt");
}

#[test]
fn router_hands_channels_to_the_tunnel_that_claimed_the_port() {
    let router = ForwardRouter::<u32>::new();
    let (guard, mut rx) = router.register(9000).expect("free port");
    assert!(router.register(9000).is_none(), "already claimed");

    assert!(router.route(9000, 7).is_ok());
    assert_eq!(rx.try_recv().unwrap(), 7);
    assert_eq!(router.route(9001, 8), Err(8), "nobody wants that port");

    drop(guard);
    assert_eq!(router.route(9000, 9), Err(9), "released");
    assert!(router.register(9000).is_some());
}

#[test]
fn keys_ignore_secrets_in_the_id_but_not_in_the_hash() {
    let plan = |password: &str| SessionPlan {
        kind: crate::core::state::SessionKind::Ssh,
        profile_id: Some("p1".into()),
        profile_name: "prod".into(),
        host: Some("h".into()),
        port: 22,
        username: Some("u".into()),
        password: Some(password.into()),
        ignore_saved_password: false,
        remember_password: false,
        keepalive_interval_secs: 30,
        keepalive_count_max: 3,
        reconnect_enabled: false,
        reconnect_max_attempts: 0,
        private_key_path: None,
        private_key_passphrase: None,
        certificate_path: None,
        use_agent: false,
        keyboard_interactive: false,
        auto_auth: false,
        agent_forward: false,
        terminal_shell: None,
        jump_hosts: Vec::new(),
        encoding: Default::default(),
        shell_integration: false,
        session_log: Default::default(),
    };
    let (a, b) = (
        SessionKey::for_plan(&plan("x")),
        SessionKey::for_plan(&plan("y")),
    );
    assert_eq!(a.id, b.id);
    assert_ne!(a.credentials, b.credentials);
    assert_eq!(a, SessionKey::for_plan(&plan("x")));
}
