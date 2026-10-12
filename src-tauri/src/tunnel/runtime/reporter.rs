//! A tunnel's status and traffic, reported to the frontend and saved.

use super::{STATUS_EVENT, TRAFFIC_SAVE_INTERVAL};
use crate::tunnel::storage::{add_tunnel_traffic, reset_tunnel_traffic, tunnel_traffic};
use crate::tunnel::types::{TunnelState, TunnelStats, TunnelStatus, TunnelTraffic};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter};

/// active, total, failed, bytes up, bytes down.
type Counters = (u64, u64, u64, u64, u64);

struct StatusInner {
    state: TunnelState,
    message: Option<String>,
    bound_port: Option<u16>,
    connected_at: Option<u64>,
    retry_attempt: u32,
    last_emitted_counters: Counters,
}

/// How much of the live counters has been saved to the database.
struct TrafficLedger {
    /// Saved totals, as of the last read or write.
    saved: TunnelTraffic,
    /// The live counters (up, down, connections) already included in `saved`.
    included: (u64, u64, u64),
    saved_at: Instant,
}

/// Holds a tunnel's live state and pushes changes to the UI.
pub struct TunnelReporter {
    app: AppHandle,
    id: String,
    pub stats: Arc<TunnelStats>,
    inner: Mutex<StatusInner>,
    ledger: Mutex<TrafficLedger>,
}

impl TunnelReporter {
    pub fn new(app: AppHandle, id: String) -> Arc<Self> {
        let saved =
            crate::db::read(|connection| tunnel_traffic(connection, &id)).unwrap_or_else(|error| {
                eprintln!("Failed to load traffic of tunnel {id}: {error}");
                TunnelTraffic::default()
            });
        Arc::new(Self {
            app,
            id,
            stats: Arc::new(TunnelStats::default()),
            inner: Mutex::new(StatusInner {
                state: TunnelState::Stopped,
                message: None,
                bound_port: None,
                connected_at: None,
                retry_attempt: 0,
                last_emitted_counters: (0, 0, 0, 0, 0),
            }),
            ledger: Mutex::new(TrafficLedger {
                saved,
                included: (0, 0, 0),
                saved_at: Instant::now(),
            }),
        })
    }

    /// The live counters that count toward the lifetime totals.
    fn live_traffic(&self) -> (u64, u64, u64) {
        (
            self.stats.bytes_up.load(Ordering::Relaxed),
            self.stats.bytes_down.load(Ordering::Relaxed),
            self.stats.total_connections.load(Ordering::Relaxed),
        )
    }

    fn ledger(&self) -> std::sync::MutexGuard<'_, TrafficLedger> {
        self.ledger.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn lifetime(&self) -> TunnelTraffic {
        let ledger = self.ledger();
        let (up, down, connections) = self.live_traffic();
        ledger.saved.plus((
            up.saturating_sub(ledger.included.0),
            down.saturating_sub(ledger.included.1),
            connections.saturating_sub(ledger.included.2),
        ))
    }

    /// Adds what the live counters gained since the last save to the saved
    /// totals. On failure the gain stays pending for the next attempt.
    fn save_traffic_locked(&self, ledger: &mut TrafficLedger) {
        let live = self.live_traffic();
        let delta = (
            live.0.saturating_sub(ledger.included.0),
            live.1.saturating_sub(ledger.included.1),
            live.2.saturating_sub(ledger.included.2),
        );
        ledger.saved_at = Instant::now();
        if delta == (0, 0, 0) {
            return;
        }
        let now = crate::ssh::now_unix_ms().max(0) as u64;
        match crate::db::write(|transaction| add_tunnel_traffic(transaction, &self.id, delta, now))
        {
            Ok(()) => {
                ledger.saved = ledger.saved.plus(delta);
                ledger.saved.since.get_or_insert(now);
                ledger.included = live;
            }
            Err(error) => eprintln!("Failed to save traffic of tunnel {}: {error}", self.id),
        }
    }

    /// Saves traffic not yet in the database.
    pub fn save_traffic(&self) {
        let mut ledger = self.ledger();
        self.save_traffic_locked(&mut ledger);
    }

    /// Zeroes the lifetime totals, the current run included.
    pub fn reset_traffic(&self) -> Result<(), String> {
        {
            let mut ledger = self.ledger();
            let now = crate::ssh::now_unix_ms().max(0) as u64;
            crate::db::write(|transaction| reset_tunnel_traffic(transaction, &self.id, now))?;
            ledger.saved = TunnelTraffic {
                since: Some(now),
                ..TunnelTraffic::default()
            };
            ledger.included = self.live_traffic();
        }
        self.emit();
        Ok(())
    }

    fn counters(&self) -> Counters {
        (
            self.stats.active_connections.load(Ordering::Relaxed),
            self.stats.total_connections.load(Ordering::Relaxed),
            self.stats.failed_connections.load(Ordering::Relaxed),
            self.stats.bytes_up.load(Ordering::Relaxed),
            self.stats.bytes_down.load(Ordering::Relaxed),
        )
    }

    pub fn state(&self) -> TunnelState {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).state
    }

    pub fn snapshot(&self) -> TunnelStatus {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (active, total, failed, up, down) = self.counters();
        TunnelStatus {
            id: self.id.clone(),
            state: inner.state,
            message: inner.message.clone(),
            bound_port: inner.bound_port,
            active_connections: active,
            total_connections: total,
            failed_connections: failed,
            bytes_up: up,
            bytes_down: down,
            connected_at: inner.connected_at,
            retry_attempt: inner.retry_attempt,
            last_failure: self.stats.last_failure(),
            lifetime: self.lifetime(),
        }
    }

    fn emit(&self) {
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.last_emitted_counters = self.counters();
        }
        let _ = self.app.emit(STATUS_EVENT, self.snapshot());
    }

    pub fn set_state(&self, state: TunnelState, message: Option<String>) {
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.state = state;
            inner.message = message;
            if state != TunnelState::Running {
                inner.connected_at = None;
            }
            if matches!(state, TunnelState::Stopped | TunnelState::Starting) {
                inner.retry_attempt = 0;
            }
            if matches!(state, TunnelState::Stopped | TunnelState::Error) {
                inner.bound_port = None;
            }
        }
        if state == TunnelState::Starting {
            // Bank the previous run before its counters are cleared.
            let mut ledger = self.ledger();
            self.save_traffic_locked(&mut ledger);
            self.stats.reset();
            ledger.included = (0, 0, 0);
        } else {
            self.save_traffic();
        }
        self.emit();
        crate::background::refresh_tunnels(&self.app);
    }

    pub(super) fn set_retry_attempt(&self, attempt: u32) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retry_attempt = attempt;
    }

    pub(super) fn set_bound_port(&self, port: Option<u16>) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .bound_port = port;
    }

    pub(super) fn set_running(&self) {
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.state = TunnelState::Running;
            inner.message = None;
            inner.connected_at = Some(crate::ssh::now_unix_ms().max(0) as u64);
        }
        self.emit();
    }

    /// Emits a status update only when traffic counters moved.
    pub(super) fn tick(&self) {
        let changed = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.last_emitted_counters != self.counters()
        };
        if changed {
            self.emit();
        }
        let mut ledger = self.ledger();
        if ledger.saved_at.elapsed() >= TRAFFIC_SAVE_INTERVAL {
            self.save_traffic_locked(&mut ledger);
        }
    }
}
