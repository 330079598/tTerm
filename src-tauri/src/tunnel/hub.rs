//! One SSH connection shared by every tunnel that goes through the same host.
//!
//! Opening a connection costs a handshake, authentication and any jump hosts,
//! and servers limit unauthenticated connections (`MaxStartups`). Tunnels to
//! one host therefore lease a single session from the hub; the last tunnel to
//! let go closes it.

use crate::core::session::SessionPlan;
use crate::ssh::{ForwardedTcpIp, JumpChain, SshClientHandler, SshConnectError};
use russh::client::Handle;
use russh::Disconnect;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

pub type SshHandle = Handle<SshClientHandler>;

/// A live connection, as far as the hub needs to know.
pub trait HubConnection: Send + Sync + 'static {
    /// The connection is gone and must not be handed out again.
    fn is_closed(&self) -> bool;
    /// Politely ends the connection.
    fn close(&self) -> impl Future<Output = ()> + Send;
}

/// An authenticated SSH session plus the jump hosts that carry it.
pub struct SshConnection {
    pub handle: Arc<SshHandle>,
    _jump_chain: Option<JumpChain>,
}

impl SshConnection {
    pub fn new(handle: SshHandle, jump_chain: Option<JumpChain>) -> Self {
        Self {
            handle: Arc::new(handle),
            _jump_chain: jump_chain,
        }
    }
}

impl HubConnection for SshConnection {
    fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }

    async fn close(&self) {
        let _ = self
            .handle
            .disconnect(Disconnect::ByApplication, "Tunnel closed", "en")
            .await;
    }
}

/// Identifies which connection a tunnel needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionKey {
    /// Where and as whom: tunnels with the same id can share a connection.
    pub id: String,
    /// Hash of the secrets in use. A failed attempt is only reused by tunnels
    /// that would have tried the same secrets, so one tunnel's mistyped
    /// password cannot fail another's good one.
    pub credentials: u64,
}

impl SessionKey {
    pub fn for_plan(plan: &SessionPlan) -> Self {
        let mut id = format!(
            "{}|{}:{}|{}|{}",
            plan.profile_id.as_deref().unwrap_or(&plan.profile_name),
            plan.host.as_deref().unwrap_or_default(),
            plan.port,
            plan.username.as_deref().unwrap_or_default(),
            plan.private_key_path.as_deref().unwrap_or_default(),
        );
        let mut hasher = DefaultHasher::new();
        plan.password.hash(&mut hasher);
        plan.private_key_passphrase.hash(&mut hasher);
        for jump in &plan.jump_hosts {
            let _ = write!(
                id,
                "|{}@{}:{}#{}",
                jump.username,
                jump.host,
                jump.port,
                jump.private_key_path.as_deref().unwrap_or_default()
            );
            jump.password.hash(&mut hasher);
            jump.private_key_passphrase.hash(&mut hasher);
        }
        Self {
            id,
            credentials: hasher.finish(),
        }
    }
}

/// Hands the channels a server opens for remote forwards to the tunnel that
/// asked for that port.
pub struct ForwardRouter<T> {
    routes: Mutex<HashMap<u16, mpsc::UnboundedSender<T>>>,
}

impl<T> ForwardRouter<T> {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            routes: Mutex::new(HashMap::new()),
        })
    }

    /// Claims `port`; `None` if another tunnel already has it.
    fn register(
        self: &Arc<Self>,
        port: u16,
    ) -> Option<(RouteGuard<T>, mpsc::UnboundedReceiver<T>)> {
        let mut routes = self.routes.lock().unwrap_or_else(|e| e.into_inner());
        if routes.get(&port).is_some_and(|sender| !sender.is_closed()) {
            return None;
        }
        let (tx, rx) = mpsc::unbounded_channel();
        routes.insert(port, tx);
        Some((
            RouteGuard {
                router: self.clone(),
                port,
            },
            rx,
        ))
    }

    /// Delivers `item`, or gives it back when nobody wants that port.
    fn route(&self, port: u16, item: T) -> Result<(), T> {
        let routes = self.routes.lock().unwrap_or_else(|e| e.into_inner());
        match routes.get(&port) {
            Some(sender) => sender.send(item).map_err(|err| err.0),
            None => Err(item),
        }
    }
}

/// Releases a claimed port when dropped.
pub struct RouteGuard<T> {
    router: Arc<ForwardRouter<T>>,
    port: u16,
}

impl<T> Drop for RouteGuard<T> {
    fn drop(&mut self) {
        self.router
            .routes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.port);
    }
}

struct Shared<C> {
    conn: C,
    /// Only changed while holding the slot's state lock.
    refs: AtomicUsize,
    router: Arc<ForwardRouter<ForwardedTcpIp>>,
    router_task: JoinHandle<()>,
}

impl<C> Drop for Shared<C> {
    fn drop(&mut self) {
        self.router_task.abort();
    }
}

struct Failure {
    at: Instant,
    credentials: u64,
    error: SshConnectError,
}

struct SlotState<C> {
    current: Option<Arc<Shared<C>>>,
    failure: Option<Failure>,
}

/// The connection for one [`SessionKey`].
struct Slot<C> {
    /// Held while dialing so concurrent tunnels wait for one attempt.
    dial: tokio::sync::Mutex<()>,
    state: Mutex<SlotState<C>>,
}

impl<C: HubConnection> Slot<C> {
    fn lock(&self) -> std::sync::MutexGuard<'_, SlotState<C>> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A lease on the live connection, if there is one.
    fn lease(self: &Arc<Self>) -> Option<Lease<C>> {
        let state = self.lock();
        let current = state.current.as_ref()?;
        if current.conn.is_closed() {
            return None;
        }
        current.refs.fetch_add(1, Ordering::SeqCst);
        Some(Lease {
            slot: self.clone(),
            shared: current.clone(),
        })
    }

    fn release(&self, shared: &Arc<Shared<C>>) {
        let last = {
            let mut state = self.lock();
            let last = shared.refs.fetch_sub(1, Ordering::SeqCst) == 1;
            if last
                && state
                    .current
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, shared))
            {
                state.current = None;
            }
            last
        };
        if last {
            // Holding the whole `Shared` keeps the jump hosts alive until the
            // goodbye has been sent.
            let shared = shared.clone();
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move { shared.conn.close().await });
            }
        }
    }
}

/// A tunnel's claim on a shared connection. Dropping it lets go; the last
/// claim closes the connection.
pub struct Lease<C: HubConnection> {
    slot: Arc<Slot<C>>,
    shared: Arc<Shared<C>>,
}

impl<C: HubConnection> Lease<C> {
    pub fn connection(&self) -> &C {
        &self.shared.conn
    }

    /// Claims a server-side listening `port` for this tunnel's remote forward.
    pub fn register_route(
        &self,
        port: u16,
    ) -> Option<(
        RouteGuard<ForwardedTcpIp>,
        mpsc::UnboundedReceiver<ForwardedTcpIp>,
    )> {
        self.shared.router.register(port)
    }
}

impl<C: HubConnection> Drop for Lease<C> {
    fn drop(&mut self) {
        self.slot.release(&self.shared);
    }
}

pub struct SessionHub<C: HubConnection> {
    slots: Mutex<HashMap<String, Arc<Slot<C>>>>,
}

impl<C: HubConnection> Default for SessionHub<C> {
    fn default() -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
        }
    }
}

impl<C: HubConnection> SessionHub<C> {
    fn slot(&self, id: &str) -> Arc<Slot<C>> {
        self.slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(id.to_string())
            .or_insert_with(|| {
                Arc::new(Slot {
                    dial: tokio::sync::Mutex::new(()),
                    state: Mutex::new(SlotState {
                        current: None,
                        failure: None,
                    }),
                })
            })
            .clone()
    }

    /// Leases the connection for `key`, dialing one with `dial` when none is
    /// live. Tunnels asking at the same time share a single attempt, and share
    /// its failure too, so a wrong password is tried once, not once per tunnel.
    ///
    /// `dial` receives the sender the connection must deliver remote-forward
    /// channels to.
    pub async fn acquire<F, Fut>(
        &self,
        key: &SessionKey,
        dial: F,
    ) -> Result<Lease<C>, SshConnectError>
    where
        F: FnOnce(mpsc::UnboundedSender<ForwardedTcpIp>) -> Fut,
        Fut: Future<Output = Result<C, SshConnectError>>,
    {
        let slot = self.slot(&key.id);
        let asked_at = Instant::now();
        if let Some(lease) = slot.lease() {
            return Ok(lease);
        }

        let _dialing = slot.dial.lock().await;
        // Whoever held the lock may have connected, or failed, meanwhile.
        if let Some(lease) = slot.lease() {
            return Ok(lease);
        }
        if let Some(failure) = &slot.lock().failure {
            if failure.at >= asked_at && failure.credentials == key.credentials {
                return Err(failure.error.clone());
            }
        }

        let (tx, mut rx) = mpsc::unbounded_channel::<ForwardedTcpIp>();
        match dial(tx).await {
            Ok(conn) => {
                let router = ForwardRouter::<ForwardedTcpIp>::new();
                let routing = router.clone();
                let router_task = tokio::spawn(async move {
                    while let Some(forwarded) = rx.recv().await {
                        let port = forwarded.connected_port;
                        if let Err(unwanted) = routing.route(port, forwarded) {
                            let _ = unwanted.channel.close().await;
                        }
                    }
                });
                let shared = Arc::new(Shared {
                    conn,
                    refs: AtomicUsize::new(1),
                    router,
                    router_task,
                });
                let mut state = slot.lock();
                state.current = Some(shared.clone());
                state.failure = None;
                drop(state);
                Ok(Lease {
                    slot: slot.clone(),
                    shared,
                })
            }
            Err(error) => {
                slot.lock().failure = Some(Failure {
                    at: Instant::now(),
                    credentials: key.credentials,
                    error: error.clone(),
                });
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
            terminal_shell: None,
            jump_hosts: Vec::new(),
        };
        let (a, b) = (
            SessionKey::for_plan(&plan("x")),
            SessionKey::for_plan(&plan("y")),
        );
        assert_eq!(a.id, b.id);
        assert_ne!(a.credentials, b.credentials);
        assert_eq!(a, SessionKey::for_plan(&plan("x")));
    }
}
