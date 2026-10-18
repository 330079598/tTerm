//! Writing the same input to several sessions at once.

use crate::core::state::{ActiveSession, PtySession};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyWriteTarget {
    pub tab_id: String,
    pub session_nonce: u32,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PtyWriteStatus {
    Written,
    Stale,
    Missing,
    Reconnecting,
    Failed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyWriteResult {
    pub tab_id: String,
    pub status: PtyWriteStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl PtyWriteResult {
    fn new(tab_id: String, status: PtyWriteStatus) -> Self {
        Self {
            tab_id,
            status,
            error: None,
        }
    }

    fn failed(tab_id: String, error: String) -> Self {
        Self {
            tab_id,
            status: PtyWriteStatus::Failed,
            error: Some(error),
        }
    }
}

pub(super) enum BatchTarget {
    Ready {
        tab_id: String,
        active: Arc<TokioMutex<Option<ActiveSession>>>,
    },
    Result(PtyWriteResult),
}

pub(super) fn snapshot_batch_targets(
    sessions: &HashMap<String, PtySession>,
    targets: Vec<PtyWriteTarget>,
) -> Vec<BatchTarget> {
    let mut seen = HashSet::with_capacity(targets.len());
    targets
        .into_iter()
        .filter(|target| seen.insert(target.tab_id.clone()))
        .map(|target| match sessions.get(&target.tab_id) {
            None => {
                BatchTarget::Result(PtyWriteResult::new(target.tab_id, PtyWriteStatus::Missing))
            }
            Some(session) if session.session_nonce != target.session_nonce => {
                BatchTarget::Result(PtyWriteResult::new(target.tab_id, PtyWriteStatus::Stale))
            }
            Some(session) => BatchTarget::Ready {
                tab_id: target.tab_id,
                active: session.active.clone(),
            },
        })
        .collect()
}

fn write_active_session(tab_id: String, active: &mut ActiveSession, data: &[u8]) -> PtyWriteResult {
    let write_result = match active {
        ActiveSession::Local(local) => local.input.send(data.to_vec()),
        ActiveSession::Ssh(ssh) => ssh.input.send(ssh.encoding.encode_input(data).into_owned()),
        ActiveSession::Serial(serial) => serial.input.send(serial.encode_input(data).into_owned()),
    };

    match write_result {
        Ok(()) => PtyWriteResult::new(tab_id, PtyWriteStatus::Written),
        Err(error) => PtyWriteResult::failed(tab_id, error.to_string()),
    }
}

fn write_batch_target(target: BatchTarget, data: &[u8]) -> PtyWriteResult {
    let (tab_id, active) = match target {
        BatchTarget::Result(result) => return result,
        BatchTarget::Ready { tab_id, active } => (tab_id, active),
    };
    let mut active_guard = active.blocking_lock();
    let Some(active) = active_guard.as_mut() else {
        return PtyWriteResult::new(tab_id, PtyWriteStatus::Reconnecting);
    };
    write_active_session(tab_id, active, data)
}

fn write_targets(targets: Vec<BatchTarget>, data: &[u8]) -> Vec<PtyWriteResult> {
    targets
        .into_iter()
        .map(|target| write_batch_target(target, data))
        .collect()
}

pub(super) fn write_guarded_batch(
    guard_target: Option<BatchTarget>,
    targets: Vec<BatchTarget>,
    data: &[u8],
) -> Vec<PtyWriteResult> {
    let Some(guard_target) = guard_target else {
        return write_targets(targets, data);
    };
    let (tab_id, active) = match guard_target {
        BatchTarget::Result(result) => return vec![result],
        BatchTarget::Ready { tab_id, active } => (tab_id, active),
    };

    // Keep the source session active until every target write has completed.
    let mut active_guard = active.blocking_lock();
    let Some(active) = active_guard.as_mut() else {
        return vec![PtyWriteResult::new(tab_id, PtyWriteStatus::Reconnecting)];
    };
    let guard_result = write_active_session(tab_id, active, data);
    if guard_result.status != PtyWriteStatus::Written {
        return vec![guard_result];
    }

    let mut results = Vec::with_capacity(targets.len() + 1);
    results.push(guard_result);
    results.extend(write_targets(targets, data));
    results
}

#[cfg(test)]
mod tests;
