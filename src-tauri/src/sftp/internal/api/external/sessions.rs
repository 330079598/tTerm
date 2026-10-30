//! The open files' bookkeeping, kept under one lock that is never held
//! across I/O.

use super::local::Fingerprint;
use super::remote::RemoteStamp;
use super::watch::{Look, Watched};
use super::{EditSession, EditStatus, ExternalEdit, ExternalEditState};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{MutexGuard, PoisonError};

/// What an upload needs from its session.
pub(super) struct UploadSource {
    pub remote_path: String,
    pub local: PathBuf,
    pub baseline: RemoteStamp,
}

impl ExternalEditState {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, EditSession>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The session already open for this file in this tab.
    pub(super) fn find(&self, tab_id: &str, remote_path: &str) -> Option<ExternalEdit> {
        self.lock()
            .values()
            .find(|session| {
                session.edit.tab_id == tab_id && session.edit.remote_path == remote_path
            })
            .map(|session| session.edit.clone())
    }

    pub(super) fn get(&self, id: &str) -> Option<ExternalEdit> {
        self.lock().get(id).map(|session| session.edit.clone())
    }

    pub(super) fn insert(&self, session: EditSession) {
        self.lock().insert(session.edit.id.clone(), session);
    }

    pub(super) fn list(&self, tab_id: &str) -> Vec<ExternalEdit> {
        let mut edits: Vec<_> = self
            .lock()
            .values()
            .filter(|session| session.edit.tab_id == tab_id)
            .map(|session| session.edit.clone())
            .collect();
        edits.sort_by(|a, b| a.remote_path.cmp(&b.remote_path));
        edits
    }

    pub(super) fn remove(&self, id: &str) -> Option<EditSession> {
        self.lock().remove(id)
    }

    /// Removes every session of the tab.
    pub(super) fn remove_tab(&self, tab_id: &str) -> Vec<EditSession> {
        let mut sessions = self.lock();
        let ids: Vec<_> = sessions
            .values()
            .filter(|session| session.edit.tab_id == tab_id)
            .map(|session| session.edit.id.clone())
            .collect();
        ids.iter().filter_map(|id| sessions.remove(id)).collect()
    }

    pub(super) fn remove_all(&self) -> Vec<EditSession> {
        self.lock().drain().map(|(_, session)| session).collect()
    }

    /// Records a fresh download into the session's copy.
    pub(super) fn refreshed(
        &self,
        id: &str,
        baseline: RemoteStamp,
        synced: Fingerprint,
    ) -> Option<ExternalEdit> {
        let mut sessions = self.lock();
        let session = sessions.get_mut(id)?;
        session.baseline = baseline;
        session.synced = synced;
        session.observed = Some(synced.stamp);
        session.settling = None;
        session.set_status(EditStatus::Synced, None);
        Some(session.edit.clone())
    }

    /// Marks the session uploading. Fails while an upload already runs.
    pub(super) fn begin_upload(&self, id: &str) -> Result<(UploadSource, ExternalEdit), String> {
        let mut sessions = self.lock();
        let session = sessions
            .get_mut(id)
            .ok_or("This file is no longer open in another program")?;
        if session.edit.status == EditStatus::Uploading {
            return Err("This file is already being uploaded".to_string());
        }
        session.set_status(EditStatus::Uploading, None);
        Ok((
            UploadSource {
                remote_path: session.edit.remote_path.clone(),
                local: session.local.clone(),
                baseline: session.baseline,
            },
            session.edit.clone(),
        ))
    }

    /// Records an upload of `uploaded` that left the remote file at `stamp`.
    /// A save made while it ran is found by the next look, as the copy no
    /// longer matches what was uploaded.
    pub(super) fn uploaded(
        &self,
        id: &str,
        stamp: RemoteStamp,
        uploaded: Fingerprint,
    ) -> Option<ExternalEdit> {
        let mut sessions = self.lock();
        let session = sessions.get_mut(id)?;
        session.baseline = stamp;
        session.synced = uploaded;
        session.observed = Some(uploaded.stamp);
        session.settling = None;
        session.set_status(EditStatus::Synced, None);
        Some(session.edit.clone())
    }

    /// Ends an upload that wrote nothing or failed: the changes are still
    /// only in the copy.
    pub(super) fn upload_stopped(&self, id: &str, error: Option<String>) -> Option<ExternalEdit> {
        let mut sessions = self.lock();
        let session = sessions.get_mut(id)?;
        let status = if error.is_some() {
            EditStatus::Failed
        } else {
            EditStatus::Modified
        };
        session.set_status(status, error);
        Some(session.edit.clone())
    }

    /// The copies to look at, or None once no file is open, which ends the
    /// watch. Deciding that under the lock means a file opened meanwhile
    /// starts a new watch rather than being missed.
    pub(super) fn watched(&self) -> Option<Vec<Watched>> {
        let sessions = self.lock();
        if sessions.is_empty() {
            self.watching.store(false, Ordering::SeqCst);
            return None;
        }
        Some(
            sessions
                .values()
                .filter(|session| session.edit.status != EditStatus::Uploading)
                .map(|session| Watched {
                    id: session.edit.id.clone(),
                    local: session.local.clone(),
                    observed: session.observed,
                    settling: session.settling,
                })
                .collect(),
        )
    }

    /// Takes in what the looks found, and returns the files whose state
    /// changed: a save that differs from the server, or one that undid the
    /// changes.
    pub(super) fn apply_looks(&self, looks: Vec<(String, Look)>) -> Vec<ExternalEdit> {
        let mut sessions = self.lock();
        let mut changed = Vec::new();
        for (id, look) in looks {
            let Some(session) = sessions.get_mut(&id) else {
                continue;
            };
            if session.edit.status == EditStatus::Uploading {
                continue;
            }
            match look {
                Look::Same => session.settling = None,
                Look::Settling(stamp) => session.settling = Some(stamp),
                Look::Settled(fingerprint) => {
                    session.observed = Some(fingerprint.stamp);
                    session.settling = None;
                    if fingerprint.hash != session.synced.hash {
                        session.set_status(EditStatus::Modified, None);
                        changed.push(session.edit.clone());
                    } else if session.edit.status != EditStatus::Synced {
                        session.set_status(EditStatus::Synced, None);
                        changed.push(session.edit.clone());
                    }
                }
            }
        }
        changed
    }
}

impl EditSession {
    fn set_status(&mut self, status: EditStatus, error: Option<String>) {
        self.edit.status = status;
        self.edit.error = error;
    }
}
