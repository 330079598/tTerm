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
use tterm_core::config::panes::PaneLayoutTemplate;
use tterm_core::config::panes_file::canonical_pane_templates;
use tterm_core::config::scrollback::normalize_scrollback_config;
use tterm_core::core::blocking::run_blocking;

/// Writes one normalized scrollback setting through the shared core path.
pub(crate) type ScrollbackWriter = Arc<dyn Fn(u32) -> Result<(), String> + Send + Sync>;

/// Writes the complete saved template list through the shared core path.
pub(crate) type PaneTemplatesWriter =
    Arc<dyn Fn(Vec<PaneLayoutTemplate>) -> Result<(), String> + Send + Sync>;

/// Emitted after the strict config file accepted a scrollback change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollbackChanged {
    /// The normalized capacity; `0` keeps unlimited history.
    pub lines: u32,
}

/// Emitted after the strict config file accepted a pane-template change.
#[derive(Clone, Debug, PartialEq)]
pub struct PaneTemplatesChanged {
    /// The complete saved template list, in display order.
    pub templates: Vec<PaneLayoutTemplate>,
}

/// The application's live configuration entity.
pub struct ConfigState {
    config: AppConfig,
    runtime: tokio::runtime::Handle,
    scrollback_writer: ScrollbackWriter,
    pane_templates_writer: PaneTemplatesWriter,
    pending: bool,
}

impl EventEmitter<ScrollbackChanged> for ConfigState {}
impl EventEmitter<PaneTemplatesChanged> for ConfigState {}

impl ConfigState {
    /// Creates the state from the strict config the root loaded and normalized
    /// before the application started.
    pub fn new(config: AppConfig, runtime: &tokio::runtime::Handle) -> Self {
        Self::with_writers(
            config,
            runtime,
            Arc::new(tterm_core::config::scrollback_file::save_scrollback_lines),
            Arc::new(tterm_core::config::panes_file::save_pane_templates),
        )
    }

    /// Creates the state with a writer other than the production config file.
    ///
    /// Cross-module tests use this to observe a save without touching the
    /// user's config. Production code should call [`ConfigState::new`].
    #[cfg(test)]
    pub(crate) fn with_writer(
        config: AppConfig,
        runtime: &tokio::runtime::Handle,
        writer: ScrollbackWriter,
    ) -> Self {
        Self::with_writers(config, runtime, writer, Arc::new(|_| Ok(())))
    }

    /// Creates the state with writers controlled by a cross-module test.
    pub(crate) fn with_writers(
        config: AppConfig,
        runtime: &tokio::runtime::Handle,
        scrollback_writer: ScrollbackWriter,
        pane_templates_writer: PaneTemplatesWriter,
    ) -> Self {
        Self {
            config,
            runtime: runtime.clone(),
            scrollback_writer,
            pane_templates_writer,
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
            return Task::ready(Err(pending_save_error()));
        }
        let lines = normalize_scrollback_config(Some(f64::from(lines)));
        if lines == self.config.scrollback_lines {
            return Task::ready(Ok(lines));
        }
        self.pending = true;

        let writer = Arc::clone(&self.scrollback_writer);
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

    /// Saves the complete list of pane layout templates.
    ///
    /// The in-memory snapshot and [`PaneTemplatesChanged`] are updated only
    /// after the strict config file write succeeds. A second edit is refused
    /// while any setting save is pending. Dropping the returned task still
    /// lets the write finish and releases the pending state.
    pub fn save_pane_templates(
        &mut self,
        templates: Vec<PaneLayoutTemplate>,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        if self.pending {
            return Task::ready(Err(pending_save_error()));
        }
        let templates = match canonical_pane_templates(&templates) {
            Ok(templates) => templates,
            Err(error) => return Task::ready(Err(error)),
        };
        if templates == self.config.panes.templates {
            return Task::ready(Ok(()));
        }
        self.pending = true;

        let writer = Arc::clone(&self.pane_templates_writer);
        let runtime = self.runtime.clone();
        let written = templates.clone();
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let _save = runtime.spawn(async move {
            let result = run_blocking(move || writer(written)).await;
            let _ = result_tx.send(result);
        });

        let (waiter_tx, waiter_rx) = tokio::sync::oneshot::channel();
        cx.spawn(async move |this, cx| {
            let result = result_rx.await.unwrap_or_else(|_| {
                Err("The pane template saver stopped before it reported a result.".to_string())
            });
            let committed = result.is_ok();
            let _ = this.update(cx, |state, cx| {
                state.pending = false;
                if committed {
                    state.config.panes.templates = templates.clone();
                    cx.emit(PaneTemplatesChanged {
                        templates: templates.clone(),
                    });
                    cx.notify();
                }
            });
            let _ = waiter_tx.send(result);
        })
        .detach();

        cx.spawn(async move |_, _| {
            waiter_rx.await.unwrap_or_else(|_| {
                Err("The pane template saver stopped before it reported a result.".to_string())
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

fn pending_save_error() -> String {
    "A settings save is already in progress. Wait for it to finish before changing a setting again."
        .to_string()
}

#[cfg(test)]
mod tests;
