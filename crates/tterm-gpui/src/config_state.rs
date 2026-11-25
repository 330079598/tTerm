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
use tterm_core::config::local_shells::LocalShell;
use tterm_core::config::local_shells_file::SavedLocalShells;
use tterm_core::config::panes::PaneLayoutTemplate;
use tterm_core::config::panes_file::canonical_pane_templates;
use tterm_core::config::scrollback::normalize_scrollback_config;
#[cfg(any(test, all(debug_assertions, feature = "automation")))]
use tterm_core::config::terminal_font_file::SavedTerminalFont;
use tterm_core::config::terminal_font_file::TerminalPageSettings;
use tterm_core::core::blocking::run_blocking;
use tterm_core::workspace::local_shells::types::DefaultShellSettings;

/// Writes one normalized scrollback setting through the shared core path.
pub(crate) type ScrollbackWriter = Arc<dyn Fn(u32) -> Result<(), String> + Send + Sync>;

/// Writes one local-shell edit through the shared core path.
pub(crate) type LocalShellsWriter =
    Arc<dyn Fn(LocalShell) -> Result<SavedLocalShells, String> + Send + Sync>;

/// Records font edits in isolated tests and captures.
#[cfg(any(test, all(debug_assertions, feature = "automation")))]
pub(crate) type TerminalFontWriter =
    Arc<dyn Fn(String, u16) -> Result<SavedTerminalFont, String> + Send + Sync>;

/// Writes the complete Terminal page as one configuration edit, for tests that
/// inject the page writer directly.
#[cfg(test)]
pub(crate) type TerminalPageWriter =
    Arc<dyn Fn(TerminalPageSettings) -> Result<TerminalPageSettings, String> + Send + Sync>;

/// Saves one page edit against the snapshot the state last published, so the
/// production writer can leave the fields that snapshot did not change alone.
type TerminalPageSaver = Arc<
    dyn Fn(AppConfig, TerminalPageSettings) -> Result<TerminalPageSettings, String> + Send + Sync,
>;

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

/// Emitted after the strict config file accepted a local-shell change.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalShellsChanged {
    /// The complete saved list, in display order and without replaced entries.
    pub local_shells: Vec<LocalShell>,
    /// The default shell settings the save also moved, when it did.
    pub default_shell: DefaultShellSettings,
}

/// Emitted after the strict config file accepted a Terminal font change.
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalFontChanged {
    /// The CSS font-family chain the file holds.
    pub family: String,
    /// The font size in logical pixels the file holds.
    pub size: u16,
    /// The line-height multiplier the file holds.
    pub line_height: f64,
    /// The extra cell spacing in pixels the file holds.
    pub letter_spacing: i8,
}

/// The application's live configuration entity.
pub struct ConfigState {
    config: AppConfig,
    runtime: tokio::runtime::Handle,
    scrollback_writer: ScrollbackWriter,
    pane_templates_writer: PaneTemplatesWriter,
    local_shells_writer: LocalShellsWriter,
    terminal_page_saver: TerminalPageSaver,
    pending: bool,
}

impl EventEmitter<ScrollbackChanged> for ConfigState {}
impl EventEmitter<PaneTemplatesChanged> for ConfigState {}
impl EventEmitter<LocalShellsChanged> for ConfigState {}
impl EventEmitter<TerminalFontChanged> for ConfigState {}

impl ConfigState {
    /// Creates the state from the strict config the root loaded and normalized
    /// before the application started.
    pub fn new(config: AppConfig, runtime: &tokio::runtime::Handle) -> Self {
        Self::from_writers(
            config,
            runtime,
            Arc::new(tterm_core::config::scrollback_file::save_scrollback_lines),
            Arc::new(tterm_core::config::panes_file::save_pane_templates),
            Arc::new(tterm_core::config::local_shells_file::save_local_shell),
            Arc::new(|previous, settings| {
                tterm_core::config::terminal_font_file::save_terminal_page(&previous, settings)
            }),
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
        let local_shells_writer: LocalShellsWriter = Arc::new(|shell| {
            Ok(SavedLocalShells {
                local_shells: vec![shell],
                default_shell: DefaultShellSettings::default(),
            })
        });
        Self::with_writers(
            config,
            runtime,
            writer,
            Arc::new(|_| Ok(())),
            local_shells_writer,
        )
    }

    /// Creates the state with writers controlled by a cross-module test or
    /// the settings capture.
    ///
    /// The Terminal font writer only reports the requested values, so
    /// neither a test nor an automation run can write the user's settings.
    #[cfg(any(test, all(debug_assertions, feature = "automation")))]
    pub(crate) fn with_writers(
        config: AppConfig,
        runtime: &tokio::runtime::Handle,
        scrollback_writer: ScrollbackWriter,
        pane_templates_writer: PaneTemplatesWriter,
        local_shells_writer: LocalShellsWriter,
    ) -> Self {
        let font_writer = echo_terminal_font_writer();
        let page_saver = composed_page_saver(Arc::clone(&scrollback_writer), font_writer.clone());
        Self::from_writers(
            config,
            runtime,
            scrollback_writer,
            pane_templates_writer,
            local_shells_writer,
            page_saver,
        )
    }

    /// Creates the state with a Terminal font writer controlled by a test.
    #[cfg(test)]
    pub(crate) fn with_terminal_font_writer(
        config: AppConfig,
        runtime: &tokio::runtime::Handle,
        writer: TerminalFontWriter,
    ) -> Self {
        let page_saver = composed_page_saver(Arc::new(|_| Ok(())), Arc::clone(&writer));
        Self::from_writers(
            config,
            runtime,
            Arc::new(|_| Ok(())),
            Arc::new(|_| Ok(())),
            echo_local_shells_writer(),
            page_saver,
        )
    }

    /// Creates the state with a Terminal page writer controlled by a test.
    ///
    /// The complete page goes through one writer, so a route test can fail or
    /// record the single edit without the legacy per-field writers.
    #[cfg(test)]
    pub(crate) fn with_terminal_page_writer(
        config: AppConfig,
        runtime: &tokio::runtime::Handle,
        writer: TerminalPageWriter,
    ) -> Self {
        let page_saver: TerminalPageSaver = Arc::new(move |_previous, settings| writer(settings));
        Self::from_writers(
            config,
            runtime,
            Arc::new(|_| Ok(())),
            Arc::new(|_| Ok(())),
            echo_local_shells_writer(),
            page_saver,
        )
    }

    fn from_writers(
        config: AppConfig,
        runtime: &tokio::runtime::Handle,
        scrollback_writer: ScrollbackWriter,
        pane_templates_writer: PaneTemplatesWriter,
        local_shells_writer: LocalShellsWriter,
        terminal_page_saver: TerminalPageSaver,
    ) -> Self {
        Self {
            config,
            runtime: runtime.clone(),
            scrollback_writer,
            pane_templates_writer,
            local_shells_writer,
            terminal_page_saver,
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

    /// Saves one local shell, adding it or replacing the entry with its id.
    ///
    /// The writer applies the shared rules to the latest config file, so a
    /// concurrent edit or an unknown field is not replaced by this window's
    /// startup snapshot. The in-memory list and [`LocalShellsChanged`] are
    /// updated only after the write succeeds. A second edit is refused while
    /// any setting save is pending. Dropping the returned task still lets the
    /// write finish and releases the pending state.
    pub fn save_local_shell(
        &mut self,
        shell: LocalShell,
        cx: &mut Context<Self>,
    ) -> Task<Result<LocalShellsChanged, String>> {
        if self.pending {
            return Task::ready(Err(pending_save_error()));
        }
        self.pending = true;

        let writer = Arc::clone(&self.local_shells_writer);
        let runtime = self.runtime.clone();
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let _save = runtime.spawn(async move {
            let result = run_blocking(move || writer(shell)).await;
            let _ = result_tx.send(result);
        });

        let (waiter_tx, waiter_rx) = tokio::sync::oneshot::channel();
        cx.spawn(async move |this, cx| {
            let result = result_rx.await.unwrap_or_else(|_| {
                Err("The local shell saver stopped before it reported a result.".to_string())
            });
            let outcome = match result {
                Ok(saved) => {
                    let changed = LocalShellsChanged {
                        local_shells: saved.local_shells,
                        default_shell: saved.default_shell,
                    };
                    let committed = changed.clone();
                    let _ = this.update(cx, |state, cx| {
                        state.pending = false;
                        state.apply_local_shells(&committed);
                        cx.emit(committed);
                        cx.notify();
                    });
                    Ok(changed)
                }
                Err(error) => {
                    let _ = this.update(cx, |state, _| state.pending = false);
                    Err(error)
                }
            };
            let _ = waiter_tx.send(outcome);
        })
        .detach();

        cx.spawn(async move |_, _| {
            waiter_rx.await.unwrap_or_else(|_| {
                Err("The local shell saver stopped before it reported a result.".to_string())
            })
        })
    }

    fn apply_local_shells(&mut self, changed: &LocalShellsChanged) {
        self.config.local_shells = changed.local_shells.clone();
        self.config.terminal_shell = changed.default_shell.terminal_shell.clone();
        self.config.terminal_shell_custom_path =
            changed.default_shell.terminal_shell_custom_path.clone();
        self.config.terminal_shell_custom_args =
            changed.default_shell.terminal_shell_custom_args.clone();
        self.config.terminal_shell_id = changed.default_shell.terminal_shell_id.clone();
        self.config.terminal_shell_wsl_distro =
            changed.default_shell.terminal_shell_wsl_distro.clone();
    }

    /// Saves the Terminal font family and size.
    ///
    /// The writer applies the change to the latest config file and reads the
    /// values back, so the snapshot and [`TerminalFontChanged`] publish what
    /// the next launch loads rather than what this window asked for. Both are
    /// updated only after the write succeeds. A second edit is refused while
    /// any setting save is pending. Dropping the returned task still lets the
    /// write finish and releases the pending state.
    #[cfg(test)]
    pub(crate) fn save_terminal_font(
        &mut self,
        family: String,
        size: u16,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        let settings = TerminalPageSettings {
            family,
            size,
            ..TerminalPageSettings::from(&self.config)
        };
        self.save_terminal_page(settings, cx)
    }

    fn apply_terminal_font(&mut self, changed: &TerminalFontChanged) {
        self.config.font_family = changed.family.clone();
        self.config.font_size = changed.size;
        self.config.terminal_line_height = changed.line_height;
        self.config.terminal_letter_spacing = changed.letter_spacing;
    }

    /// Saves every Terminal page field as one configuration edit.
    ///
    /// The draft covers the font family, size, line height, letter spacing and
    /// scrollback capacity, so pressing Enter in any field writes the whole
    /// page rather than only the focused input. The draft is checked with the
    /// shared core rules before a writer runs. The writer applies only the
    /// fields that differ from this state's snapshot to the latest config
    /// file and returns the values the file now holds; the snapshot, the
    /// [`TerminalFontChanged`] event and, when the capacity moved, the
    /// [`ScrollbackChanged`] event are published only after that succeeds. A
    /// second edit is refused while any setting save is pending.
    pub fn save_terminal_page(
        &mut self,
        settings: TerminalPageSettings,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        if self.pending {
            return Task::ready(Err(pending_save_error()));
        }
        let settings = match settings.checked() {
            Ok(settings) => settings,
            Err(error) => return Task::ready(Err(error)),
        };
        if settings == TerminalPageSettings::from(&self.config) {
            return Task::ready(Ok(()));
        }
        self.pending = true;

        let previous = self.config.clone();
        let previous_lines = self.config.scrollback_lines;
        let saver = Arc::clone(&self.terminal_page_saver);
        let runtime = self.runtime.clone();
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let _save = runtime.spawn(async move {
            let result = run_blocking(move || saver(previous, settings)).await;
            let _ = result_tx.send(result);
        });

        let (waiter_tx, waiter_rx) = tokio::sync::oneshot::channel();
        cx.spawn(async move |this, cx| {
            let result = result_rx
                .await
                .unwrap_or_else(|_| Err(terminal_page_saver_stopped()));
            let outcome = match result {
                Ok(saved) => {
                    let scrollback_lines = saved.scrollback_lines;
                    let changed = TerminalFontChanged {
                        family: saved.family,
                        size: saved.size,
                        line_height: saved.line_height,
                        letter_spacing: saved.letter_spacing,
                    };
                    let published = changed.clone();
                    let _ = this.update(cx, |state, cx| {
                        state.pending = false;
                        state.apply_terminal_font(&published);
                        state.config.scrollback_lines = scrollback_lines;
                        cx.emit(published);
                        if scrollback_lines != previous_lines {
                            cx.emit(ScrollbackChanged {
                                lines: scrollback_lines,
                            });
                        }
                        cx.notify();
                    });
                    Ok(())
                }
                Err(error) => {
                    let _ = this.update(cx, |state, _| state.pending = false);
                    Err(error)
                }
            };
            let _ = waiter_tx.send(outcome);
        })
        .detach();

        cx.spawn(async move |_, _| {
            waiter_rx
                .await
                .unwrap_or_else(|_| Err(terminal_page_saver_stopped()))
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

fn terminal_page_saver_stopped() -> String {
    "The terminal page saver stopped before it reported a result.".to_string()
}

/// Combines the legacy per-field writers into one page edit for tests and
/// captures that do not use the isolated page writer.
#[cfg(any(test, all(debug_assertions, feature = "automation")))]
fn composed_page_saver(
    scrollback_writer: ScrollbackWriter,
    font_writer: TerminalFontWriter,
) -> TerminalPageSaver {
    Arc::new(move |_previous, settings| {
        scrollback_writer(settings.scrollback_lines)?;
        let saved = font_writer(settings.family.clone(), settings.size)?;
        Ok(TerminalPageSettings {
            family: saved.family,
            size: saved.size,
            ..settings
        })
    })
}

/// A writer for tests that do not persist settings: it keeps the saved shell
/// alone and reports the platform default.
#[cfg(test)]
pub(crate) fn echo_local_shells_writer() -> LocalShellsWriter {
    Arc::new(|shell| {
        Ok(SavedLocalShells {
            local_shells: vec![shell],
            default_shell: DefaultShellSettings::default(),
        })
    })
}

/// A writer for tests and captures that do not persist settings: it reports
/// the requested font back without touching a config file.
#[cfg(any(test, all(debug_assertions, feature = "automation")))]
fn echo_terminal_font_writer() -> TerminalFontWriter {
    Arc::new(|family, size| Ok(SavedTerminalFont { family, size }))
}

#[cfg(test)]
mod terminal_page_tests;
#[cfg(test)]
mod tests;
