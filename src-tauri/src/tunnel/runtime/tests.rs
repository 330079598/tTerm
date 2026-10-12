use super::connections::{ChannelFailure, ConnectionGuard, Counted};
use super::*;
use crate::tunnel::socks5::ConnectOutcome;
use crate::tunnel::types::TunnelStats;
use std::sync::atomic::Ordering;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[test]
fn backoff_grows_and_is_capped() {
    assert_eq!(backoff_delay(1), Duration::from_secs(2));
    assert_eq!(backoff_delay(3), Duration::from_secs(8));
    assert_eq!(backoff_delay(5), Duration::from_secs(30));
    assert_eq!(backoff_delay(40), Duration::from_secs(30));
}

#[test]
fn limiter_refuses_past_the_limit_and_frees_slots() {
    let stats = TunnelStats::default();
    let limiter = ConnectionLimiter::new(2);
    let a = limiter.acquire(&stats).expect("first slot");
    let _b = limiter.acquire(&stats).expect("second slot");
    assert!(limiter.acquire(&stats).is_none());
    assert_eq!(stats.failed_connections.load(Ordering::Relaxed), 1);

    drop(a);
    assert!(limiter.acquire(&stats).is_some());
}

#[test]
fn remote_star_means_all_interfaces() {
    assert_eq!(remote_bind_address("*"), "");
    assert_eq!(remote_bind_address("localhost"), "localhost");
    assert_eq!(remote_bind_address("0.0.0.0"), "0.0.0.0");
}

async fn free_port() -> u16 {
    let probe = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    probe.local_addr().unwrap().port()
}

#[tokio::test]
async fn localhost_listens_on_ipv4_and_serves_accepts() {
    let port = free_port().await;
    let listeners = Listeners::bind("localhost", port).await.unwrap();
    assert_eq!(listeners.local_port(), Some(port));

    let _client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let (_, peer) = listeners.accept().await.unwrap();
    assert!(peer.ip().is_loopback());
}

#[tokio::test]
async fn binding_a_taken_port_fails() {
    let port = free_port().await;
    let _first = Listeners::bind("127.0.0.1", port).await.unwrap();
    assert!(Listeners::bind("127.0.0.1", port).await.is_err());
}

#[tokio::test]
async fn refused_connections_are_closed_at_once() {
    use tokio::io::AsyncReadExt;
    let port = free_port().await;
    let listeners = Listeners::bind("127.0.0.1", port).await.unwrap();
    let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();

    let mut buf = [0u8; 1];
    let read = tokio::select! {
        never = refuse_connections(Some(&listeners)) => match never {},
        read = tokio::time::timeout(Duration::from_secs(2), client.read(&mut buf)) => read,
    };
    assert_eq!(read.expect("client must not hang").unwrap_or(0), 0);
}

#[test]
fn keepalive_is_clamped_for_tunnels() {
    assert_eq!(tunnel_keepalive(0, 0), (30, 3));
    assert_eq!(tunnel_keepalive(120, 10), (30, 3));
    assert_eq!(tunnel_keepalive(10, 2), (10, 2));
}

#[test]
fn channel_failures_are_classified_and_described() {
    use russh::ChannelOpenFailure as Reason;
    let classify = |reason| ChannelFailure::from_error(russh::Error::ChannelOpenFailure(reason));

    assert_eq!(
        classify(Reason::AdministrativelyProhibited),
        ChannelFailure::Prohibited
    );
    assert_eq!(
        classify(Reason::ConnectFailed),
        ChannelFailure::ConnectFailed
    );
    assert_eq!(
        classify(Reason::ResourceShortage),
        ChannelFailure::ResourceShortage
    );
    assert!(matches!(
        classify(Reason::UnknownChannelType),
        ChannelFailure::Other(_)
    ));

    let message = ChannelFailure::ConnectFailed.describe("db.internal", 5432);
    assert!(message.starts_with("db.internal:5432: "));
}

#[test]
fn channel_failures_map_to_socks_replies() {
    assert!(matches!(
        ChannelFailure::Prohibited.socks_outcome(),
        ConnectOutcome::NotAllowed
    ));
    assert!(matches!(
        ChannelFailure::ConnectFailed.socks_outcome(),
        ConnectOutcome::Refused
    ));
    assert!(matches!(
        ChannelFailure::TimedOut.socks_outcome(),
        ConnectOutcome::TimedOut
    ));
    assert!(matches!(
        ChannelFailure::Other("x".into()).socks_outcome(),
        ConnectOutcome::Failed
    ));
}

#[test]
fn connection_guard_tracks_active_and_total() {
    let stats = Arc::new(TunnelStats::default());
    {
        let _a = ConnectionGuard::new(&stats);
        let _b = ConnectionGuard::new(&stats);
        assert_eq!(stats.active_connections.load(Ordering::Relaxed), 2);
    }
    assert_eq!(stats.active_connections.load(Ordering::Relaxed), 0);
    assert_eq!(stats.total_connections.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn counted_stream_attributes_bytes_by_direction() {
    for (local_is_client, up, down) in [(true, 5, 3), (false, 3, 5)] {
        let stats = Arc::new(TunnelStats::default());
        let (near, mut far) = tokio::io::duplex(64);
        let mut counted = Counted::new(near, stats.clone(), local_is_client);

        far.write_all(b"hello").await.unwrap();
        let mut buf = [0u8; 5];
        counted.read_exact(&mut buf).await.unwrap();
        counted.write_all(b"abc").await.unwrap();

        assert_eq!(stats.bytes_up.load(Ordering::Relaxed), up);
        assert_eq!(stats.bytes_down.load(Ordering::Relaxed), down);
    }
}

#[tokio::test]
async fn wait_for_stop_resolves_on_signal_and_when_sender_drops() {
    let (tx, mut rx) = watch::channel(false);
    tx.send(true).unwrap();
    wait_for_stop(&mut rx).await;

    let (tx, mut rx) = watch::channel(false);
    drop(tx);
    wait_for_stop(&mut rx).await;
}
