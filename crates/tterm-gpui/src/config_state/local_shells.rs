//! Local-shell saves and the cached list of shells found on this machine.

use std::sync::Arc;

use gpui_kit::{Context, Task};
use tterm_core::config::local_shells::LocalShell;
use tterm_core::config::local_shells_file::{SavedLocalShells, SavedShellLists, ShellListEdit};
use tterm_core::core::blocking::run_blocking;
use tterm_core::terminal::pty::TerminalShellProfile;
use tterm_core::workspace::local_shells::types::LocalShellChoice;

use super::{ConfigState, LocalShellsChanged, pending_save_error};

const LOCAL_SHELL_SAVER_STOPPED: &str =
    "The local shell saver stopped before it reported a result.";
const SHELL_LIST_SAVER_STOPPED: &str = "The shell list saver stopped before it reported a result.";
const DEFAULT_SHELL_SAVER_STOPPED: &str =
    "The default shell saver stopped before it reported a result.";

impl ConfigState {
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
        let hidden_shells = self.config.hidden_shells.clone();
        self.finish_local_shells_save(
            result_rx,
            LOCAL_SHELL_SAVER_STOPPED,
            move |saved: SavedLocalShells| LocalShellsChanged {
                local_shells: saved.local_shells,
                hidden_shells,
                default_shell: saved.default_shell,
            },
            cx,
        )
    }

    /// Applies one move, removal or visibility change to the shell lists.
    ///
    /// The writer re-reads the latest config file under the shared edit lock,
    /// so the edit never replaces a concurrent change or a key this version
    /// does not know. The complete local, hidden and default snapshot the file
    /// now holds is committed to this state and emitted only after the write
    /// succeeds. A second edit is refused while any setting save is pending.
    pub fn edit_shell_list(
        &mut self,
        edit: ShellListEdit,
        cx: &mut Context<Self>,
    ) -> Task<Result<LocalShellsChanged, String>> {
        if self.pending {
            return Task::ready(Err(pending_save_error()));
        }
        self.pending = true;

        let writer = Arc::clone(&self.shell_list_writer);
        let runtime = self.runtime.clone();
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let _save = runtime.spawn(async move {
            let result = run_blocking(move || writer(edit)).await;
            let _ = result_tx.send(result);
        });
        self.finish_local_shells_save(
            result_rx,
            SHELL_LIST_SAVER_STOPPED,
            |saved: SavedShellLists| LocalShellsChanged {
                local_shells: saved.local_shells,
                hidden_shells: saved.hidden_shells,
                default_shell: saved.default_shell,
            },
            cx,
        )
    }

    /// Saves `choice` as the default shell.
    ///
    /// The writer reads the latest config file, so a default that names one of
    /// the user's shells follows that shell's current path and arguments, and
    /// a shell removed by another edit is refused instead of saved as an
    /// orphan. The in-memory settings and [`LocalShellsChanged`] are updated
    /// only after the write succeeds. A second edit is refused while any
    /// setting save is pending.
    pub fn set_default_shell(
        &mut self,
        choice: LocalShellChoice,
        cx: &mut Context<Self>,
    ) -> Task<Result<LocalShellsChanged, String>> {
        if self.pending {
            return Task::ready(Err(pending_save_error()));
        }
        self.pending = true;

        let writer = Arc::clone(&self.default_shell_writer);
        let runtime = self.runtime.clone();
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let _save = runtime.spawn(async move {
            let result = run_blocking(move || writer(choice)).await;
            let _ = result_tx.send(result);
        });
        let hidden_shells = self.config.hidden_shells.clone();
        self.finish_local_shells_save(
            result_rx,
            DEFAULT_SHELL_SAVER_STOPPED,
            move |saved: SavedLocalShells| LocalShellsChanged {
                local_shells: saved.local_shells,
                hidden_shells,
                default_shell: saved.default_shell,
            },
            cx,
        )
    }

    fn finish_local_shells_save<T: Send + 'static>(
        &mut self,
        result_rx: tokio::sync::oneshot::Receiver<Result<T, String>>,
        stopped: &'static str,
        to_changed: impl FnOnce(T) -> LocalShellsChanged + Send + 'static,
        cx: &mut Context<Self>,
    ) -> Task<Result<LocalShellsChanged, String>> {
        let (waiter_tx, waiter_rx) = tokio::sync::oneshot::channel();
        cx.spawn(async move |this, cx| {
            let result = result_rx.await.unwrap_or_else(|_| Err(stopped.to_string()));
            let outcome = match result {
                Ok(saved) => {
                    let changed = to_changed(saved);
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

        cx.spawn(async move |_, _| waiter_rx.await.unwrap_or_else(|_| Err(stopped.to_string())))
    }

    fn apply_local_shells(&mut self, changed: &LocalShellsChanged) {
        self.config.local_shells = changed.local_shells.clone();
        self.config.hidden_shells = changed.hidden_shells.clone();
        self.config.terminal_shell = changed.default_shell.terminal_shell.clone();
        self.config.terminal_shell_custom_path =
            changed.default_shell.terminal_shell_custom_path.clone();
        self.config.terminal_shell_custom_args =
            changed.default_shell.terminal_shell_custom_args.clone();
        self.config.terminal_shell_id = changed.default_shell.terminal_shell_id.clone();
        self.config.terminal_shell_wsl_distro =
            changed.default_shell.terminal_shell_wsl_distro.clone();
    }

    /// The shells found on this machine, once discovery has completed.
    pub fn detected_shells(&self) -> &[TerminalShellProfile] {
        &self.detected_shells
    }

    /// Starts the one discovery this state performs.
    ///
    /// The probe runs on the shared Tokio runtime, so a caller may invoke this
    /// from an action but never from render. Later calls reuse the stored list
    /// instead of probing again; only a successful probe notifies the state.
    pub fn discover_shells(&mut self, cx: &mut Context<Self>) {
        if self.shell_discovery_started {
            return;
        }
        self.shell_discovery_started = true;

        let runtime = self.runtime.clone();
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let _probe = runtime.spawn(async move {
            let result = tterm_core::terminal::pty::list_available_terminal_shells_service().await;
            let _ = result_tx.send(result);
        });

        cx.spawn(async move |this, cx| {
            let Ok(Ok(profiles)) = result_rx.await else {
                return;
            };
            let _ = this.update(cx, |state, cx| {
                state.detected_shells = profiles;
                cx.notify();
            });
        })
        .detach();
    }

    /// Replaces the list writer in a test that must observe or fail an edit.
    #[cfg(test)]
    pub(crate) fn set_shell_list_writer_for_test(&mut self, writer: super::ShellListWriter) {
        self.shell_list_writer = writer;
    }

    /// Records shells for a test without probing this machine.
    #[cfg(test)]
    pub(crate) fn set_detected_shells_for_test(&mut self, profiles: Vec<TerminalShellProfile>) {
        self.detected_shells = profiles;
        self.shell_discovery_started = true;
    }
}
