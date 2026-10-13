//! One SSH connection shared by every tunnel that goes through the same host.
//!
//! Opening a connection costs a handshake, authentication and any jump hosts,
//! and servers limit unauthenticated connections (`MaxStartups`). Tunnels to
//! one host therefore lease a single session from the hub; the last tunnel to
//! let go closes it.

use crate::core::session::types::SessionPlan;
use crate::ssh::{
    jump::connect::JumpChain, types::ForwardedTcpIp, types::SshClientHandler,
    types::SshConnectError,
};
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
            "{}|{}:{}|{}|{}|{}",
            plan.profile_id.as_deref().unwrap_or(&plan.profile_name),
            plan.host.as_deref().unwrap_or_default(),
            plan.port,
            plan.username.as_deref().unwrap_or_default(),
            plan.private_key_path.as_deref().unwrap_or_default(),
            plan.certificate_path.as_deref().unwrap_or_default(),
        );
        let mut hasher = DefaultHasher::new();
        plan.password.hash(&mut hasher);
        plan.private_key_passphrase.hash(&mut hasher);
        for jump in &plan.jump_hosts {
            let _ = write!(
                id,
                "|{}@{}:{}#{}#{}",
                jump.username,
                jump.host,
                jump.port,
                jump.private_key_path.as_deref().unwrap_or_default(),
                jump.certificate_path.as_deref().unwrap_or_default()
            );
            jump.password.hash(&mut hasher);
            jump.private_key_passphrase.hash(&mut hasher);
        }
        // Tunnels share a session only when they reach it the same way.
        if let Some(proxy) = &plan.proxy {
            let _ = write!(
                id,
                "|proxy {:?} {}@{}:{}",
                proxy.kind,
                proxy.username.as_deref().unwrap_or_default(),
                proxy.host,
                proxy.port
            );
            proxy.password.as_deref().hash(&mut hasher);
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
mod tests;
