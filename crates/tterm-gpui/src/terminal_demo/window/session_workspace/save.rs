//! Debounced and final workspace writes on one FIFO task.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};
use tterm_core::session_restore::workspace::WorkspaceSnapshot;

use super::{WorkspaceFuture, shared_slot, write_global};

/// Matches the Tauri client's one-second save debounce.
pub(super) const SAVE_DEBOUNCE: Duration = Duration::from_millis(1000);

type Writer = Arc<dyn Fn(WorkspaceSnapshot) -> WorkspaceFuture + Send + Sync>;

/// Owns the latest snapshot and the serialized writer. A disabled instance
/// keeps the snapshot in memory but never writes a session it cannot safely
/// replace.
#[derive(Clone)]
pub(in crate::terminal_demo::window) struct WorkspacePersistence {
    shared: Arc<Mutex<Option<WorkspaceSnapshot>>>,
    queue: Option<WorkspaceQueue>,
    can_save: bool,
}

impl WorkspacePersistence {
    pub(super) fn start(runtime: &Handle, can_save: bool) -> Self {
        Self::start_with_writer(runtime, can_save, Arc::new(write_global))
    }

    fn start_with_writer(runtime: &Handle, can_save: bool, writer: Writer) -> Self {
        Self {
            shared: shared_slot(),
            queue: can_save.then(|| WorkspaceQueue::start(runtime, writer)),
            can_save,
        }
    }

    pub(super) fn can_save(&self) -> bool {
        self.can_save
    }

    pub(super) fn publish(&self, snapshot: Option<WorkspaceSnapshot>) {
        if let Ok(mut shared) = self.shared.lock() {
            *shared = snapshot;
        }
    }

    pub(super) fn latest(&self) -> Option<WorkspaceSnapshot> {
        self.shared.lock().ok().and_then(|value| value.clone())
    }

    pub(super) fn queue(&self) -> Option<WorkspaceQueue> {
        self.queue.clone()
    }

    pub(super) async fn flush(&self, snapshot: WorkspaceSnapshot) -> Result<(), String> {
        self.publish(Some(snapshot.clone()));
        match &self.queue {
            Some(queue) => queue.flush(snapshot).await,
            None => Ok(()),
        }
    }

    pub(super) async fn flush_latest(&self) -> Result<(), String> {
        match self.latest() {
            Some(snapshot) => self.flush(snapshot).await,
            None => Ok(()),
        }
    }
}

struct SaveRequest {
    snapshot: WorkspaceSnapshot,
    acknowledge: oneshot::Sender<Result<(), String>>,
}

/// Serializes writes on one background task. A flush waits for its own write,
/// so earlier saves always finish first.
#[derive(Clone)]
pub(super) struct WorkspaceQueue {
    sender: mpsc::UnboundedSender<SaveRequest>,
}

impl WorkspaceQueue {
    fn start(runtime: &Handle, writer: Writer) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel::<SaveRequest>();
        runtime.spawn(async move {
            while let Some(request) = receiver.recv().await {
                let writer = Arc::clone(&writer);
                let result = writer(request.snapshot).await;
                let _ = request.acknowledge.send(result);
            }
        });
        Self { sender }
    }

    pub(super) async fn flush(&self, snapshot: WorkspaceSnapshot) -> Result<(), String> {
        let (acknowledge, outcome) = oneshot::channel();
        self.sender
            .send(SaveRequest {
                snapshot,
                acknowledge,
            })
            .map_err(|_| "The workspace saver has stopped".to_string())?;
        outcome
            .await
            .map_err(|_| "The workspace saver dropped the save".to_string())?
    }
}

#[cfg(test)]
pub(super) fn test_persistence(
    runtime: &Handle,
    can_save: bool,
    writer: impl Fn(WorkspaceSnapshot) -> WorkspaceFuture + Send + Sync + 'static,
) -> WorkspacePersistence {
    WorkspacePersistence::start_with_writer(runtime, can_save, Arc::new(writer))
}
