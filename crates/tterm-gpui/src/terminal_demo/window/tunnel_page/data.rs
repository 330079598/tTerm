//! Read-only tunnel rules and live statuses for the port-forwarding page.
//!
//! Loading starts only when the route opens the page. Every read runs through
//! the existing core services on the blocking/runtime pool, and a single
//! visible-only poll keeps the rows current without starting or changing a
//! tunnel.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::{Context, Task};
use tterm_core::events::tunnel::{TunnelState, TunnelStatus};
use tterm_core::profiles::launch::SavedSshProfileSummary;
use tterm_core::services::AppServices;
use tterm_core::tunnel::types::TunnelRule;

mod load;
#[cfg(test)]
mod tests;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

type LoadFuture = Pin<Box<dyn Future<Output = Result<load::TunnelPageSnapshot, String>> + Send>>;
type Loader = Arc<dyn Fn() -> LoadFuture + Send + Sync>;

/// What the page shows while it has no complete snapshot yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TunnelPageState {
    Loading,
    Ready,
    Error(String),
}

/// One saved tunnel with the live status and host display data it needs.
#[derive(Debug, Clone)]
pub(super) struct TunnelRow {
    pub rule: TunnelRule,
    pub status: TunnelStatus,
    /// The saved host's name, or `user@host` when its name is blank.
    pub host_label: String,
    /// The rule points at a profile that is not in the saved profile list.
    pub host_missing: bool,
}

/// Everything the tunnel page renders, in saved rule order.
#[derive(Debug, Clone)]
pub(super) struct TunnelPageModel {
    pub state: TunnelPageState,
    pub rules: Vec<TunnelRow>,
    pub running_count: usize,
}

impl Default for TunnelPageModel {
    fn default() -> Self {
        Self {
            state: TunnelPageState::Loading,
            rules: Vec::new(),
            running_count: 0,
        }
    }
}

/// The tunnel page's read-only data entity.
pub(super) struct TunnelPageData {
    loader: Loader,
    model: TunnelPageModel,
    visible: bool,
    generation: u64,
    in_flight: bool,
    refresh_pending: bool,
    pending_statuses: HashMap<String, TunnelStatus>,
    poll_task: Option<Task<()>>,
    poll_interval: Duration,
}

impl TunnelPageData {
    /// Builds the entity without reading the database, network or keychain.
    pub(super) fn new(services: Arc<AppServices>, _cx: &mut Context<Self>) -> Self {
        Self::with_loader(
            Arc::new(move || {
                let services = Arc::clone(&services);
                Box::pin(load::load_snapshot(services))
            }),
            POLL_INTERVAL,
        )
    }

    fn with_loader(loader: Loader, poll_interval: Duration) -> Self {
        Self {
            loader,
            model: TunnelPageModel::default(),
            visible: false,
            generation: 0,
            in_flight: false,
            refresh_pending: false,
            pending_statuses: HashMap::new(),
            poll_task: None,
            poll_interval,
        }
    }

    pub(super) fn model(&self) -> &TunnelPageModel {
        &self.model
    }

    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }

    /// Marks the page visible and starts its first read and poll.
    pub(super) fn open(&mut self, cx: &mut Context<Self>) {
        if self.visible {
            self.refresh(cx);
            return;
        }
        self.visible = true;
        self.start_load(cx);
    }

    /// Marks the page hidden and discards any result still in flight.
    pub(super) fn close(&mut self, cx: &mut Context<Self>) {
        if !self.visible {
            return;
        }
        self.visible = false;
        self.generation = self.generation.wrapping_add(1);
        self.in_flight = false;
        self.refresh_pending = false;
        self.pending_statuses.clear();
        self.poll_task.take();
        cx.notify();
    }

    /// Reloads a visible page. A request already in flight is followed by one
    /// more load instead of overlapping it.
    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        if !self.visible {
            return;
        }
        if self.in_flight {
            self.refresh_pending = true;
            return;
        }
        self.start_load(cx);
    }

    /// Applies one forwarded `tunnel-status` event without reading storage.
    pub(super) fn apply_status(&mut self, status: TunnelStatus, cx: &mut Context<Self>) {
        if !self.is_visible() {
            return;
        }
        if self.in_flight {
            self.pending_statuses
                .insert(status.id.clone(), status.clone());
        }
        if self.model.state != TunnelPageState::Ready {
            return;
        }
        let Some(row) = self
            .model
            .rules
            .iter_mut()
            .find(|row| row.rule.id == status.id)
        else {
            return;
        };
        if row.status == status {
            return;
        }
        row.status = status;
        self.model.running_count = running_count(&self.model.rules);
        cx.notify();
    }

    fn start_load(&mut self, cx: &mut Context<Self>) {
        self.poll_task.take();
        self.generation = self.generation.wrapping_add(1);
        self.model = TunnelPageModel::default();
        self.in_flight = true;
        self.refresh_pending = false;
        self.pending_statuses.clear();
        let generation = self.generation;
        let loader = Arc::clone(&self.loader);
        let interval = self.poll_interval;
        self.poll_task = Some(cx.spawn(async move |this, cx| {
            loop {
                let result = loader().await;
                let (keep_polling, pending) =
                    match this.update(cx, |this, cx| this.finish_load(generation, result, cx)) {
                        Ok(value) => value,
                        Err(_) => break,
                    };
                if !keep_polling {
                    break;
                }
                if pending {
                    if this
                        .update(cx, |this, _| {
                            this.in_flight = true;
                        })
                        .is_err()
                    {
                        break;
                    }
                    continue;
                }
                cx.background_executor().timer(interval).await;
                let keep_polling = this
                    .update(cx, |this, _| {
                        if !this.visible || this.generation != generation {
                            return false;
                        }
                        this.in_flight = true;
                        true
                    })
                    .unwrap_or(false);
                if !keep_polling {
                    break;
                }
            }
        }));
    }

    /// Returns whether polling should continue and whether a coalesced refresh
    /// is waiting. A closed page or a newer request drops the result.
    fn finish_load(
        &mut self,
        generation: u64,
        result: Result<load::TunnelPageSnapshot, String>,
        cx: &mut Context<Self>,
    ) -> (bool, bool) {
        if !self.visible || generation != self.generation {
            return (false, false);
        }
        self.in_flight = false;
        let pending = std::mem::take(&mut self.refresh_pending);
        self.model = match result {
            Ok(snapshot) => {
                let mut model = load::build_model(snapshot);
                for status in self.pending_statuses.drain().map(|(_, status)| status) {
                    if let Some(row) = model.rules.iter_mut().find(|row| row.rule.id == status.id) {
                        row.status = status;
                    }
                }
                model.running_count = running_count(&model.rules);
                model
            }
            Err(error) => TunnelPageModel {
                state: TunnelPageState::Error(error),
                rules: Vec::new(),
                running_count: 0,
            },
        };
        cx.notify();
        (true, pending)
    }
}

fn running_count(rules: &[TunnelRow]) -> usize {
    rules
        .iter()
        .filter(|row| row.status.state == TunnelState::Running)
        .count()
}

fn host_label(profile: Option<&SavedSshProfileSummary>) -> String {
    let Some(profile) = profile else {
        return String::new();
    };
    if !profile.name.trim().is_empty() {
        return profile.name.clone();
    }
    format!(
        "{}@{}",
        profile.username.as_deref().unwrap_or_default(),
        profile.host.as_deref().unwrap_or_default()
    )
}
