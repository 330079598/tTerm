//! Pending monitor connections are distinct from the terminal sharing their tab.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tterm_core::prompts::{PromptBroker, PromptOwner, SessionKey};

type Owners = HashMap<String, Vec<(u32, PromptOwner)>>;
#[derive(Default)]
pub(crate) struct PendingMonitorPrompts(Mutex<Owners>);

pub(super) struct MonitorPromptScope<'a> {
    registry: &'a PendingMonitorPrompts,
    tab_id: String,
    nonce: u32,
    pub scope: crate::prompt_scope::PromptScope,
}

impl PendingMonitorPrompts {
    pub(super) fn begin(
        &self,
        broker: Arc<PromptBroker>,
        tab_id: &str,
        nonce: u32,
    ) -> Result<MonitorPromptScope<'_>, String> {
        let mut entries = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let scope = crate::prompt_scope::PromptScope::new(
            broker,
            Some(SessionKey {
                tab_id: tab_id.to_string(),
                session_nonce: nonce,
            }),
        )?;
        entries
            .entry(tab_id.to_string())
            .or_default()
            .push((nonce, scope.owner.clone()));
        Ok(MonitorPromptScope {
            registry: self,
            tab_id: tab_id.to_string(),
            nonce,
            scope,
        })
    }

    pub(crate) fn cancel(&self, broker: &PromptBroker, tab_id: &str, nonce: Option<u32>) {
        let mut entries = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(owners) = entries.get_mut(tab_id) {
            owners.retain(|(generation, owner)| {
                if nonce.is_none_or(|nonce| nonce == *generation) {
                    broker.cancel_owner(owner);
                    false
                } else {
                    true
                }
            });
        }
    }
}

impl Drop for MonitorPromptScope<'_> {
    fn drop(&mut self) {
        let mut entries = self.registry.0.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(owners) = entries.get_mut(&self.tab_id) {
            // An ended scope is closed independently; keep any concurrent request.
            self.scope.broker.cancel_owner(&self.scope.owner);
            owners.retain(|(nonce, owner)| *nonce != self.nonce || !owner.is_closed());
            if owners.is_empty() {
                entries.remove(&self.tab_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monitor_release_cancels_only_matching_monitor_owners() {
        let broker = Arc::new(PromptBroker::new(Arc::new(
            tterm_core::events::recording::RecordingEventSink::default(),
        )));
        let registry = PendingMonitorPrompts::default();
        broker.open_session(&SessionKey {
            tab_id: "tab".into(),
            session_nonce: 1,
        });
        let old = registry.begin(broker.clone(), "tab", 1).unwrap();
        let terminal = broker
            .register_owner(Some(SessionKey {
                tab_id: "tab".into(),
                session_nonce: 1,
            }))
            .unwrap();
        broker.open_session(&SessionKey {
            tab_id: "tab".into(),
            session_nonce: 2,
        });
        let new = registry.begin(broker.clone(), "tab", 2).unwrap();
        registry.cancel(&broker, "tab", Some(1));
        assert!(old.scope.owner.is_closed());
        assert!(!new.scope.owner.is_closed());
        assert!(!terminal.is_closed());
        registry.cancel(&broker, "tab", None);
        assert!(new.scope.owner.is_closed());
        assert!(!terminal.is_closed());
    }
}
