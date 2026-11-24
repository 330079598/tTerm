//! Workspace restore and persistence for the native window.
//!
//! Saved tabs keep their string ids and canonical fields. The window uses
//! independent numeric keys for its own collections, so this module is the
//! single boundary that translates between the two.

pub(super) mod launch;
mod legacy;
pub(super) mod restore;
pub(super) mod save;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use gpui_kit::{App, Context, Window};
use save::WorkspacePersistence;
use tokio::runtime::Handle;
use tterm_core::agent_status::AgentResumeSession;
use tterm_core::session_restore::scrollback::SavedScrollback;
use tterm_core::session_restore::workspace::{
    WorkspaceLoad, WorkspaceSnapshot, load_workspace_service, save_workspace_service,
};

use super::Prototype;

/// The final workspace write used by the close gate.
pub(super) type WorkspaceFuture =
    Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'static>>;

/// Loads the saved workspace before the GPUI event loop starts.
pub(super) fn load_startup(runtime: &Handle) -> WorkspaceLoad {
    match runtime.block_on(load_workspace_service()) {
        Ok(load) => load,
        Err(error) => {
            eprintln!("tterm-gpui: failed to load the workspace: {error}");
            WorkspaceLoad::Unreadable { reason: error }
        }
    }
}

/// A loaded workspace can only be written when decoding lost nothing.
pub(super) fn can_save(load: &WorkspaceLoad) -> bool {
    match load {
        WorkspaceLoad::Empty => true,
        WorkspaceLoad::Readable { can_save, .. } => *can_save,
        WorkspaceLoad::Unreadable { .. } | WorkspaceLoad::Future { .. } => false,
    }
}

/// The tabs and groups a readable workspace restored, if any.
pub(super) fn snapshot(load: WorkspaceLoad) -> Option<WorkspaceSnapshot> {
    match load {
        WorkspaceLoad::Readable { snapshot, .. } => Some(snapshot),
        _ => None,
    }
}

/// The shared final write for the quit fallback.
pub(super) fn shared_slot() -> Arc<Mutex<Option<WorkspaceSnapshot>>> {
    Arc::new(Mutex::new(None))
}

fn write_global(snapshot: WorkspaceSnapshot) -> WorkspaceFuture {
    Box::pin(save_workspace_service(snapshot))
}

/// The workspace state loaded before the window is built.
pub(super) struct StartupWorkspace {
    pub load: WorkspaceLoad,
    pub persistence: WorkspacePersistence,
    pub restore: bool,
    /// The Tauri-era snapshots to replay, keyed by the tab they belong to.
    pub legacy: HashMap<String, SavedScrollback>,
}

/// Loads the saved workspace for a bare launch, or disables persistence for
/// an explicit `--ssh`/`--exec` run.
pub(super) fn startup(
    runtime: &Handle,
    restore: bool,
    restore_terminal_content: bool,
) -> StartupWorkspace {
    if !restore {
        return StartupWorkspace {
            load: WorkspaceLoad::Empty,
            persistence: WorkspacePersistence::start(runtime, false),
            restore: false,
            legacy: HashMap::new(),
        };
    }
    let load = load_startup(runtime);
    let savable = can_save(&load);
    let legacy = if restore_terminal_content {
        legacy::read_saved(runtime, &restored_tab_ids(&load))
    } else {
        HashMap::new()
    };
    StartupWorkspace {
        load,
        persistence: WorkspacePersistence::start(runtime, savable),
        restore: true,
        legacy,
    }
}

fn restored_tab_ids(load: &WorkspaceLoad) -> Vec<String> {
    match load {
        WorkspaceLoad::Readable { snapshot, .. } => {
            snapshot.tabs.iter().map(|tab| tab.id.clone()).collect()
        }
        WorkspaceLoad::Empty | WorkspaceLoad::Unreadable { .. } | WorkspaceLoad::Future { .. } => {
            Vec::new()
        }
    }
}

/// Best-effort fallback for quit paths that bypass the window close gate.
pub(super) fn install_quit_flush(persistence: WorkspacePersistence, cx: &mut App) {
    cx.on_app_quit(move |_| {
        let persistence = persistence.clone();
        async move {
            if let Err(error) = persistence.flush_latest().await {
                eprintln!("tterm-gpui: failed to save the workspace: {error}");
            }
        }
    })
    .detach();
}

#[cfg(test)]
pub(super) fn test_startup(runtime: &Handle) -> StartupWorkspace {
    StartupWorkspace {
        load: WorkspaceLoad::Empty,
        persistence: WorkspacePersistence::start(runtime, false),
        restore: false,
        legacy: HashMap::new(),
    }
}

impl Prototype {
    /// Builds the canonical tabs and pane groups, translating internal keys
    /// back to the persisted ids. This runs on lifecycle changes, never from
    /// `render`, so terminal output does not clone the workspace.
    pub(super) fn workspace_snapshot(&self, cx: &App) -> Option<WorkspaceSnapshot> {
        let locale = self.ui.locale.read(cx);
        let resume_enabled = self.factory.config.read(cx).config().resume_agent_sessions;
        let active_tab_id = Some(self.tabs.active().saved_id.clone());
        let mut tabs = Vec::with_capacity(self.tabs.len());
        let mut ids = HashMap::with_capacity(self.tabs.len());
        for (internal, tab) in self.tabs.ordered() {
            ids.insert(internal, tab.saved_id.clone());
            let mut saved = tab.saved().clone();
            saved.set_id(tab.saved_id.clone());
            saved.set_title(tab.display_title(locale));
            if tab.may_resume_agent()
                && let Some(connection) = saved.connection.clone()
            {
                let mut connection = connection;
                let resume = resume_enabled
                    .then(|| tab.agent_resume())
                    .flatten()
                    .and_then(|resume| serde_json::to_value(resume).ok());
                connection.set_agent_resume(resume);
                saved.set_connection(Some(connection));
            }
            tabs.push(saved);
        }
        Some(WorkspaceSnapshot {
            tabs,
            active_tab_id,
            groups: self.splits.saved_groups(|id| ids.get(&id).cloned()),
        })
    }

    /// Mirrors the latest snapshot for the quit fallback. Normal closes build
    /// and flush a fresh snapshot instead of trusting this slot.
    pub(super) fn publish_workspace(&self, cx: &App) {
        self.workspace.publish(self.workspace_snapshot(cx));
    }

    /// Applies the restored layout to the views built for it.
    ///
    /// A terminal view starts visible, so every restored tab is treated as
    /// previously visible. The first refresh can then hide the views covered
    /// by a restored zoom and activate the panes that are actually in sight.
    pub(super) fn initialize_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let restored = self.tabs.ordered().map(|(id, _)| id).collect();
        self.refresh_panes(restored, window, cx);
        self.mark_workspace_dirty(cx);
    }

    /// Coalesces workspace changes for one second before saving.
    pub(super) fn mark_workspace_dirty(&mut self, cx: &mut Context<Self>) -> bool {
        if self.closing || !self.workspace.can_save() {
            return false;
        }
        self.publish_workspace(cx);
        self.schedule_workspace_save(cx);
        true
    }

    fn schedule_workspace_save(&mut self, cx: &mut Context<Self>) {
        if self.closing || !self.workspace.can_save() || self.workspace_timer.is_some() {
            return;
        }
        self.workspace_timer = Some(cx.spawn(async move |prototype, cx| {
            cx.background_executor().timer(save::SAVE_DEBOUNCE).await;
            let _ = prototype.update(cx, |prototype, cx| {
                prototype.workspace_timer = None;
                if prototype.closing {
                    return;
                }
                prototype.save_workspace(cx);
            });
        }));
    }

    fn save_workspace(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        let Some(snapshot) = self.workspace_snapshot(cx) else {
            return;
        };
        self.workspace.publish(Some(snapshot.clone()));
        let Some(queue) = self.workspace.queue() else {
            return;
        };
        cx.spawn(async move |_, _| {
            if let Err(error) = queue.flush(snapshot).await {
                eprintln!("tterm-gpui: failed to save the workspace: {error}");
            }
        })
        .detach();
    }

    /// A persisted id that no current tab owns.
    pub(super) fn next_saved_id(&mut self) -> String {
        let tabs = &self.tabs;
        Self::take_saved_id(&mut self.next_saved_tab, |id| {
            tabs.ordered().any(|(_, tab)| tab.saved_id() == id)
        })
    }

    fn take_saved_id(next: &mut u64, is_taken: impl Fn(&str) -> bool) -> String {
        loop {
            let number = (*next).max(1);
            *next = if number == u64::MAX { 1 } else { number + 1 };
            let id = format!("tab-{number}");
            if !is_taken(&id) {
                return id;
            }
        }
    }

    pub(super) fn set_agent_resume(
        &mut self,
        tab: usize,
        resume: Option<AgentResumeSession>,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab) {
            tab.set_agent_resume(resume);
            self.mark_workspace_dirty(cx);
        }
    }

    /// Flushes the workspace before geometry, then removes the window. The
    /// platform close callback and the automation endpoint share this gate so
    /// a close cannot overtake a debounced or final save.
    pub(super) fn begin_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.closing {
            return false;
        }
        self.closing = true;
        self.workspace_timer.take();
        let snapshot = self.workspace_snapshot(cx);
        self.workspace.publish(snapshot.clone());
        let (geometry, geometry_queue) = {
            let geometry = self.ui.geometry.read(cx);
            (geometry.latest(), geometry.queue())
        };
        let workspace = self.workspace.clone();
        let window_handle = window.window_handle();
        cx.spawn(async move |_, cx| {
            if let Some(snapshot) = snapshot
                && let Err(error) = workspace.flush(snapshot).await
            {
                eprintln!("tterm-gpui: failed to save the workspace: {error}");
            }
            if let Some(geometry) = geometry
                && let Err(error) = geometry_queue.flush(geometry).await
            {
                eprintln!("tterm-gpui: failed to save the window state: {error}");
            }
            cx.update(|cx| {
                let _ = window_handle.update(cx, |_, window, _| window.remove_window());
            });
        })
        .detach();
        false
    }
}
