use super::*;
use crate::core::state::AtomicTerminalSize;
use tokio::sync::{mpsc, watch};

fn session(session_nonce: u32, runtime: &tokio::runtime::Runtime) -> PtySession {
    let (stop_tx, _) = watch::channel(false);
    PtySession {
        pid: 0,
        session_nonce,
        active: Arc::new(TokioMutex::new(None)),
        stop_tx,
        supervisor: runtime.spawn(async {}),
        size: Arc::new(AtomicTerminalSize::new(24, 80)),
        output: crate::terminal::io_batcher::TerminalOutput::new(tauri::ipc::Channel::new(|_| {
            Ok(())
        })),
        serial_view: None,
    }
}

/// A broadcast target, its session slot, and what it receives as input.
type WritableTarget = (
    BatchTarget,
    Arc<TokioMutex<Option<ActiveSession>>>,
    mpsc::UnboundedReceiver<Vec<u8>>,
);

fn writable_ssh_target(runtime: &tokio::runtime::Runtime) -> WritableTarget {
    let (input_tx, input_rx) = mpsc::unbounded_channel();
    let (resize_tx, _) = mpsc::unbounded_channel();
    let active = Arc::new(TokioMutex::new(Some(ActiveSession::Ssh(
        crate::core::state::ActiveSsh {
            input: crate::terminal::types::PtyInput::over_channel(input_tx.clone()),
            input_tx,
            resize_tx,
            task: runtime.spawn(async {}),
            output_tail: Default::default(),
            encoding: Default::default(),
        },
    ))));
    (
        BatchTarget::Ready {
            tab_id: "target".to_string(),
            active: active.clone(),
        },
        active,
        input_rx,
    )
}

#[test]
fn snapshot_deduplicates_and_classifies_targets_in_request_order() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let sessions = HashMap::from([
        ("ready".to_string(), session(7, &runtime)),
        ("stale".to_string(), session(9, &runtime)),
    ]);
    let targets = vec![
        PtyWriteTarget {
            tab_id: "missing".to_string(),
            session_nonce: 1,
        },
        PtyWriteTarget {
            tab_id: "ready".to_string(),
            session_nonce: 7,
        },
        PtyWriteTarget {
            tab_id: "stale".to_string(),
            session_nonce: 8,
        },
        PtyWriteTarget {
            tab_id: "ready".to_string(),
            session_nonce: 7,
        },
    ];

    let snapshot = snapshot_batch_targets(&sessions, targets);
    assert_eq!(snapshot.len(), 3);
    assert!(matches!(
        &snapshot[0],
        BatchTarget::Result(PtyWriteResult {
            tab_id,
            status: PtyWriteStatus::Missing,
            ..
        }) if tab_id == "missing"
    ));
    assert!(matches!(
        &snapshot[1],
        BatchTarget::Ready { tab_id, .. } if tab_id == "ready"
    ));
    assert!(matches!(
        &snapshot[2],
        BatchTarget::Result(PtyWriteResult {
            tab_id,
            status: PtyWriteStatus::Stale,
            ..
        }) if tab_id == "stale"
    ));
}

#[test]
fn failed_guard_prevents_target_writes() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let guard = BatchTarget::Result(PtyWriteResult::new(
        "source".to_string(),
        PtyWriteStatus::Missing,
    ));
    let (target, target_active, mut input_rx) = writable_ssh_target(&runtime);

    let results = write_guarded_batch(Some(guard), vec![target], b"input");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tab_id, "source");
    assert_eq!(results[0].status, PtyWriteStatus::Missing);
    assert!(target_active.blocking_lock().is_some());
    assert!(matches!(
        input_rx.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
}

#[test]
fn disconnected_guard_prevents_target_writes() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let guard = BatchTarget::Ready {
        tab_id: "source".to_string(),
        active: Arc::new(TokioMutex::new(None)),
    };
    let (target, target_active, mut input_rx) = writable_ssh_target(&runtime);

    let results = write_guarded_batch(Some(guard), vec![target], b"input");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tab_id, "source");
    assert_eq!(results[0].status, PtyWriteStatus::Reconnecting);
    assert!(target_active.blocking_lock().is_some());
    assert!(matches!(
        input_rx.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
}

#[test]
fn writable_guard_receives_input_without_targets() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let (guard, guard_active, mut input_rx) = writable_ssh_target(&runtime);

    let results = write_guarded_batch(Some(guard), Vec::new(), b"input");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, PtyWriteStatus::Written);
    assert!(guard_active.blocking_lock().is_some());
    // Typed input reaches the channel from the session's input thread.
    assert_eq!(input_rx.blocking_recv().expect("guard input"), b"input");
}
