//! Shares SSH sessions between the connections that tunnel through them.
//!
//! A jump host one connection already logged in to, or a host a terminal tab
//! is connected to, carries the next connection that jumps through it, so the
//! user does not log in to it, or type its one-time code, again. A session
//! stays open while anything rides on it; the pool only remembers it.

use crate::proxy::ProxyEndpoint;
use russh::{client, Channel};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, LazyLock, Mutex, Weak};

/// An authenticated session other connections can open tunnels through.
#[async_trait::async_trait]
pub trait Relay: Send + Sync {
    /// Opens a `direct-tcpip` channel to `host:port` from the far end.
    async fn open_direct_tcpip(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Channel<client::Msg>, russh::Error>;

    fn is_closed(&self) -> bool;
}

/// A session, kept with the session it was reached through so that one stays
/// open for as long as this one is used.
pub struct PooledSession<H: client::Handler> {
    pub handle: client::Handle<H>,
    _via: Option<Arc<dyn Relay>>,
}

impl<H: client::Handler> PooledSession<H> {
    pub fn new(handle: client::Handle<H>, via: Option<Arc<dyn Relay>>) -> Self {
        Self { handle, _via: via }
    }
}

#[async_trait::async_trait]
impl<H> Relay for PooledSession<H>
where
    H: client::Handler + Send + Sync + 'static,
{
    async fn open_direct_tcpip(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Channel<client::Msg>, russh::Error> {
        self.handle
            .channel_open_direct_tcpip(host, u32::from(port), "127.0.0.1", 0)
            .await
    }

    fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }
}

/// Where a session ends up and the way it gets there: two connections share a
/// session only when they reach the same `user@host:port` over the same route.
/// How the user logged in is left out, since either login is the same user.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouteKey(String);

impl RouteKey {
    /// The key of the first host, reached through `proxy` when one is set.
    pub fn first(proxy: Option<&ProxyEndpoint>, username: &str, host: &str, port: u16) -> Self {
        let mut key = String::new();
        if let Some(proxy) = proxy {
            let _ = write!(
                key,
                "proxy {:?} {}@{}:{} {}|",
                proxy.kind,
                proxy.username.as_deref().unwrap_or_default(),
                proxy.host,
                proxy.port,
                proxy.command.as_deref().unwrap_or_default()
            );
        }
        let _ = write!(key, "{username}@{host}:{port}");
        Self(key)
    }

    /// The key of `username@host:port` reached through the session of `self`.
    pub fn then(&self, username: &str, host: &str, port: u16) -> Self {
        Self(format!("{} > {username}@{host}:{port}", self.0))
    }
}

/// The route of every hop of a chain, in order, ending with the target.
pub fn chain_keys<'a>(
    proxy: Option<&ProxyEndpoint>,
    hops: impl IntoIterator<Item = (&'a str, &'a str, u16)>,
) -> Vec<RouteKey> {
    let mut keys: Vec<RouteKey> = Vec::new();
    for (username, host, port) in hops {
        let key = match keys.last() {
            Some(previous) => previous.then(username, host, port),
            None => RouteKey::first(proxy, username, host, port),
        };
        keys.push(key);
    }
    keys
}

static SESSIONS: LazyLock<Mutex<HashMap<RouteKey, Weak<dyn Relay>>>> =
    LazyLock::new(Default::default);

/// Connections being made, so a second one to the same place waits for the
/// first instead of logging in again. Restoring several tabs behind one jump
/// host would otherwise ask for its password once per tab.
static CONNECTING: LazyLock<Mutex<HashMap<RouteKey, Weak<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(Default::default);

fn sessions() -> std::sync::MutexGuard<'static, HashMap<RouteKey, Weak<dyn Relay>>> {
    SESSIONS.lock().unwrap_or_else(|e| e.into_inner())
}

/// The open session at `key`, if one is.
pub fn find(key: &RouteKey) -> Option<Arc<dyn Relay>> {
    let mut sessions = sessions();
    let found = sessions
        .get(key)
        .and_then(Weak::upgrade)
        .filter(|session| !session.is_closed());
    if found.is_none() {
        sessions.remove(key);
    }
    found
}

/// Offers `session` to later connections that go through `key`.
pub fn offer<T: Relay + 'static>(key: RouteKey, session: &Arc<T>) {
    let weak: Weak<T> = Arc::downgrade(session);
    let weak: Weak<dyn Relay> = weak;
    let mut sessions = sessions();
    // Gone entries are dropped here so the map cannot grow without bound.
    sessions.retain(|_, entry| entry.strong_count() > 0);
    sessions.insert(key, weak);
}

/// Takes `session` out of the pool, so no later connection picks it up.
pub fn evict(key: &RouteKey, session: &Arc<dyn Relay>) {
    let mut sessions = sessions();
    if sessions
        .get(key)
        .is_some_and(|entry| std::ptr::addr_eq(entry.as_ptr(), Arc::as_ptr(session)))
    {
        sessions.remove(key);
    }
}

/// Withdraws `session` from the pool when its caller is the last one using it,
/// and says whether it was. The check and the removal happen under the pool's
/// lock, which every `find` takes too, so nobody picks it up in between.
pub fn withdraw_if_unshared<T: Relay + 'static>(key: &RouteKey, session: &Arc<T>) -> bool {
    let mut sessions = sessions();
    if Arc::strong_count(session) > 1 {
        return false;
    }
    if sessions
        .get(key)
        .is_some_and(|entry| std::ptr::addr_eq(entry.as_ptr(), Arc::as_ptr(session)))
    {
        sessions.remove(key);
    }
    true
}

/// Waits until no other connection to `key` is being made, then holds that
/// place until the guard is dropped.
pub async fn connecting(key: &RouteKey) -> tokio::sync::OwnedMutexGuard<()> {
    let lock = {
        let mut connecting = CONNECTING.lock().unwrap_or_else(|e| e.into_inner());
        connecting.retain(|_, entry| entry.strong_count() > 0);
        match connecting.get(key).and_then(Weak::upgrade) {
            Some(lock) => lock,
            None => {
                let lock = Arc::new(tokio::sync::Mutex::new(()));
                connecting.insert(key.clone(), Arc::downgrade(&lock));
                lock
            }
        }
    };
    lock.lock_owned().await
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;

    #[async_trait::async_trait]
    impl Relay for Fake {
        async fn open_direct_tcpip(
            &self,
            _host: &str,
            _port: u16,
        ) -> Result<Channel<client::Msg>, russh::Error> {
            Err(russh::Error::Disconnect)
        }

        fn is_closed(&self) -> bool {
            false
        }
    }

    #[test]
    fn a_hop_key_names_the_route_before_it() {
        let keys = chain_keys(None, [("jump", "bastion", 22), ("root", "db", 2222)]);
        assert_eq!(keys[0], RouteKey::first(None, "jump", "bastion", 22));
        assert_eq!(keys[1], keys[0].then("root", "db", 2222));
        // The same host reached another way is another session.
        assert_ne!(keys[1], RouteKey::first(None, "root", "db", 2222));
    }

    #[test]
    fn a_session_is_found_until_its_last_user_lets_go() {
        let key = RouteKey::first(None, "pool-test", "found", 22);
        let session = Arc::new(Fake);
        offer(key.clone(), &session);
        let borrowed = find(&key).expect("offered session");
        assert!(!withdraw_if_unshared(&key, &session));
        drop(borrowed);
        assert!(withdraw_if_unshared(&key, &session));
        assert!(find(&key).is_none());
    }

    #[test]
    fn a_dropped_session_is_not_found() {
        let key = RouteKey::first(None, "pool-test", "dropped", 22);
        let session = Arc::new(Fake);
        offer(key.clone(), &session);
        drop(session);
        assert!(find(&key).is_none());
    }

    #[test]
    fn an_evicted_session_is_not_found_but_its_replacement_stays() {
        let key = RouteKey::first(None, "pool-test", "evicted", 22);
        let stale = Arc::new(Fake);
        offer(key.clone(), &stale);
        let found = find(&key).expect("offered session");
        evict(&key, &found);
        assert!(find(&key).is_none());

        // A fresh login offered meanwhile is not taken out with the stale one.
        let fresh = Arc::new(Fake);
        offer(key.clone(), &fresh);
        evict(&key, &found);
        assert!(find(&key).is_some());
    }

    #[tokio::test]
    async fn a_second_connection_waits_for_the_first() {
        let key = RouteKey::first(None, "pool-test", "connecting", 22);
        let first = connecting(&key).await;
        let waiting = tokio::spawn({
            let key = key.clone();
            async move {
                let _guard = connecting(&key).await;
            }
        });
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());
        drop(first);
        waiting.await.expect("second connection proceeds");
    }
}
