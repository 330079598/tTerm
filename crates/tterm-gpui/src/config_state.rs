//! Native GPUI configuration state.
//!
//! The root loads the strict config, applies the shared UI normalization and
//! passes the snapshot here before the application starts. A setting change
//! reaches the in-memory snapshot only after the strict config file accepted
//! it, so a failed write cannot publish an irreversible change such as a
//! smaller scrollback buffer. The write runs on the shared core blocking
//! worker; the UI thread only queues it.

use std::sync::Arc;

use gpui_kit::{Context, EventEmitter, Task};
use tterm_core::config::AppConfig;
use tterm_core::config::scrollback::normalize_scrollback_config;
use tterm_core::core::blocking::run_blocking;

/// Writes one normalized scrollback setting through the shared core path.
pub(crate) type ScrollbackWriter = Arc<dyn Fn(u32) -> Result<(), String> + Send + Sync>;

/// Emitted after the strict config file accepted a scrollback change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollbackChanged {
    /// The normalized capacity; `0` keeps unlimited history.
    pub lines: u32,
}

/// The application's live configuration entity.
pub struct ConfigState {
    config: AppConfig,
    runtime: tokio::runtime::Handle,
    writer: ScrollbackWriter,
    pending: bool,
}

impl EventEmitter<ScrollbackChanged> for ConfigState {}

impl ConfigState {
    /// Creates the state from the strict config the root loaded and normalized
    /// before the application started.
    pub fn new(config: AppConfig, runtime: &tokio::runtime::Handle) -> Self {
        Self::with_writer(
            config,
            runtime,
            Arc::new(tterm_core::config::scrollback_file::save_scrollback_lines),
        )
    }

    /// Creates the state with a writer other than the production config file.
    ///
    /// Cross-module tests use this to observe a save without touching the
    /// user's config. Production code should call [`ConfigState::new`].
    pub(crate) fn with_writer(
        config: AppConfig,
        runtime: &tokio::runtime::Handle,
        writer: ScrollbackWriter,
    ) -> Self {
        Self {
            config,
            runtime: runtime.clone(),
            writer,
            pending: false,
        }
    }

    /// The current configuration snapshot.
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// The saved scrollback capacity; `0` means unlimited.
    pub fn scrollback_lines(&self) -> u32 {
        self.config.scrollback_lines
    }

    /// Saves `lines` as the scrollback capacity.
    ///
    /// The value is normalized with the shared core rules. The in-memory
    /// snapshot and [`ScrollbackChanged`] are updated only after the strict
    /// config file write succeeds, because shrinking the buffer evicts
    /// history irreversibly. A second edit is refused while one is pending.
    /// Dropping the returned task still lets the write finish and releases
    /// the pending state instead of leaving the entity locked.
    pub fn save_scrollback_lines(
        &mut self,
        lines: u32,
        cx: &mut Context<Self>,
    ) -> Task<Result<u32, String>> {
        if self.pending {
            return Task::ready(Err(
                "A scrollback save is already in progress. Wait for it to finish before changing the setting again."
                    .to_string(),
            ));
        }
        let lines = normalize_scrollback_config(Some(f64::from(lines)));
        if lines == self.config.scrollback_lines {
            return Task::ready(Ok(lines));
        }
        self.pending = true;

        let writer = Arc::clone(&self.writer);
        let runtime = self.runtime.clone();
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let _save = runtime.spawn(async move {
            let result = run_blocking(move || writer(lines)).await;
            let _ = result_tx.send(result);
        });

        let (waiter_tx, waiter_rx) = tokio::sync::oneshot::channel();
        cx.spawn(async move |this, cx| {
            let result = result_rx.await.unwrap_or_else(|_| {
                Err("The scrollback saver stopped before it reported a result.".to_string())
            });
            let committed = result.is_ok();
            let _ = this.update(cx, |state, cx| {
                state.pending = false;
                if committed {
                    state.config.scrollback_lines = lines;
                    cx.emit(ScrollbackChanged { lines });
                    cx.notify();
                }
            });
            let _ = waiter_tx.send(result.map(|()| lines));
        })
        .detach();

        cx.spawn(async move |_, _| {
            waiter_rx.await.unwrap_or_else(|_| {
                Err("The scrollback saver stopped before it reported a result.".to_string())
            })
        })
    }

    /// Whether a save is still queued or waiting to apply its result.
    ///
    /// Tests use this to prove a dropped task releases the entity instead of
    /// leaving every later edit refused.
    #[cfg(test)]
    pub(crate) fn is_save_pending(&self) -> bool {
        self.pending
    }
}

#[cfg(test)]
mod tests;
