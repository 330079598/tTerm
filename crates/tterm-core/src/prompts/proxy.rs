use super::state::{Shared, expired_error};
use super::{PromptBroker, PromptId, PromptKind, PromptOutcome, PromptReply, PromptRequest};
use std::sync::Arc;

/// A claimed proxy group retained while its caller persists the approval.
/// Dropping an unfinished claim rejects its remaining waiters.
pub struct ProxyResolution {
    shared: Arc<Shared>,
    generation: u64,
    request: PromptRequest,
}

impl PromptBroker {
    /// Returns the original first question for each group awaiting UI input.
    pub fn pending_proxy(&self) -> Vec<PromptRequest> {
        self.shared
            .lock()
            .entries
            .values()
            .filter(|entry| entry.request.kind == PromptKind::ProxyCommand && !entry.resolving)
            .map(|entry| entry.request.clone())
            .collect()
    }

    /// Hides a group before DB work, without holding a lock during that work.
    pub fn claim_proxy(&self, id: &str) -> Result<ProxyResolution, String> {
        let mut state = self.shared.lock();
        let kind = PromptKind::ProxyCommand;
        let key = (kind, PromptId(id.into()));
        let generation = state
            .active
            .remove(&key)
            .ok_or_else(|| expired_error(kind))?;
        let entry = state
            .entries
            .get_mut(&generation)
            .ok_or_else(|| expired_error(kind))?;
        entry.resolving = true;
        Ok(ProxyResolution {
            shared: self.shared.clone(),
            generation,
            request: entry.request.clone(),
        })
    }
}

impl ProxyResolution {
    /// Gives the caller its domain event/payload for the existing approval store.
    pub fn request(&self) -> &PromptRequest {
        &self.request
    }

    /// Releases waiters only after the caller has persisted an allowed command.
    pub fn complete(self, allowed: bool) {
        self.finish(allowed);
    }

    fn finish(&self, allowed: bool) {
        let entry = self.shared.lock().take(self.generation);
        if let Some(entry) = entry {
            for waiter in entry.waiters.into_values() {
                let _ = waiter
                    .sender
                    .send(PromptOutcome::Answered(PromptReply::ProxyCommand(allowed)));
            }
        }
    }
}

impl Drop for ProxyResolution {
    fn drop(&mut self) {
        self.finish(false);
    }
}
