use super::state::{Entry, Shared, Waiter, expired_error};
use super::{
    OwnerState, PromptBroker, PromptKind, PromptOptions, PromptOutcome, PromptOwner, PromptReply,
    PromptRequest, PromptWait, SessionKey,
};
use crate::events::EventSink;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

impl PromptBroker {
    /// Creates an empty broker; client transports remain behind EventSink.
    pub fn new(events: Arc<dyn EventSink>) -> Self {
        Self {
            shared: Arc::new(Shared {
                pending: Mutex::default(),
                events,
            }),
        }
    }

    /// Starts a terminal generation while the client still holds its session lock.
    /// Side operations cannot reopen a closed session by merely registering an owner.
    pub fn open_session(&self, session: &SessionKey) {
        let mut state = self.shared.lock();
        state.closed_sessions.remove(session);
        state
            .latest_sessions
            .insert(session.tab_id.clone(), session.clone());
    }

    /// Retains a closed generation so late side operations cannot become anonymous.
    pub fn session_for_tab(&self, tab_id: &str) -> Option<SessionKey> {
        self.shared.lock().latest_sessions.get(tab_id).cloned()
    }

    /// Mints an owner for one operation, optionally tied to a terminal nonce.
    pub fn register_owner(&self, session: Option<SessionKey>) -> Result<PromptOwner, String> {
        let mut state = self.shared.lock();
        if session
            .as_ref()
            .is_some_and(|session| state.closed_sessions.contains(session))
        {
            return Err("Prompt session is closed".into());
        }
        if state.shutdown {
            return Err("Prompt broker is shut down".into());
        }
        let id = state.next_id()?;
        let inner = Arc::new(OwnerState {
            id,
            session,
            closed: AtomicBool::new(false),
            broker: Arc::downgrade(&self.shared),
        });
        state.owners.retain(|_, owner| owner.strong_count() > 0);
        state.owners.insert(id, Arc::downgrade(&inner));
        Ok(PromptOwner { inner })
    }

    /// Registers before emitting, so an immediate reentrant reply cannot be lost.
    /// Transport errors retain the legacy behavior of waiting for the user timeout.
    pub fn begin(
        &self,
        request: PromptRequest,
        options: PromptOptions,
    ) -> Result<PromptWait, String> {
        let (sender, receiver) = oneshot::channel();
        let key = (request.kind, request.id.clone());
        let (generation, waiter, event) = {
            let mut state = self.shared.lock();
            if state.shutdown || options.owner.is_closed() {
                return Err("Prompt operation is closed".into());
            }
            if !options
                .owner
                .inner
                .broker
                .ptr_eq(&Arc::downgrade(&self.shared))
            {
                return Err("Prompt owner belongs to another broker".into());
            }
            if request.id.0.is_empty() {
                return Err("Prompt request ID is empty".into());
            }
            let existing = state.active.get(&key).copied();
            if existing.is_some() && request.kind != PromptKind::ProxyCommand {
                return Err("Prompt request ID is already pending".into());
            }
            let waiter = state.next_id()?;
            let generation = existing.unwrap_or(waiter);
            let event = existing.is_none().then(|| request.event.clone());
            if existing.is_none() {
                state.active.insert(key, generation);
                state.entries.insert(
                    generation,
                    Entry {
                        request: request.clone(),
                        waiters: HashMap::new(),
                        resolving: false,
                    },
                );
            }
            let entry = state
                .entries
                .get_mut(&generation)
                .ok_or("Pending prompt is unavailable")?;
            entry.waiters.insert(
                waiter,
                Waiter {
                    owner: options.owner.inner.clone(),
                    sender,
                },
            );
            (generation, waiter, event)
        };
        let wait = PromptWait {
            shared: self.shared.clone(),
            generation,
            waiter,
            id: request.id,
            receiver: Some(receiver),
            timeout: options.timeout,
            first_waiter: event.is_some(),
        };
        if let Some(event) = event {
            let _ = self.shared.events.emit(event);
        }
        Ok(wait)
    }

    /// Consumes exactly one host-key question using the existing command errors.
    pub fn reply_host_key(&self, id: &str, trust: bool) -> Result<(), String> {
        self.reply(id, PromptKind::HostKey, PromptReply::HostKey(trust))
    }

    /// None cancels the question without closing the operation itself.
    pub fn reply_auth(&self, id: &str, responses: Option<Vec<String>>) -> Result<(), String> {
        self.reply(id, PromptKind::Auth, PromptReply::Auth(responses))
    }

    fn reply(&self, id: &str, kind: PromptKind, reply: PromptReply) -> Result<(), String> {
        let entry = {
            let mut state = self.shared.lock();
            let generation = state
                .active
                .get(&(kind, super::PromptId(id.into())))
                .copied()
                .ok_or_else(|| expired_error(kind))?;
            state.take(generation).ok_or_else(|| expired_error(kind))?
        };
        let sender = entry
            .waiters
            .into_values()
            .next()
            .ok_or_else(|| expired_error(kind))?
            .sender;
        sender.send(PromptOutcome::Answered(reply)).map_err(|_| {
            match kind {
                PromptKind::HostKey => "Host key prompt receiver is gone",
                _ => "Authentication prompt receiver is gone",
            }
            .to_string()
        })
    }

    /// Closes only this operation, including requests already claimed for approval.
    pub fn cancel_owner(&self, owner: &PromptOwner) {
        if owner.inner.broker.ptr_eq(&Arc::downgrade(&self.shared)) {
            self.cancel_matching(|candidate| candidate.id == owner.inner.id, None);
        }
    }

    /// A stale terminal close never cancels a new nonce of the same tab.
    pub fn cancel_session(&self, session: &SessionKey) {
        self.cancel_matching(
            |owner| owner.session.as_ref() == Some(session),
            Some(session),
        );
    }

    /// Called only at committed application shutdown, not when asking to quit.
    pub fn shutdown(&self) {
        {
            let mut state = self.shared.lock();
            state.shutdown = true;
        }
        self.cancel_matching(|_| true, None);
    }

    fn cancel_matching(&self, matches: impl Fn(&OwnerState) -> bool, session: Option<&SessionKey>) {
        let senders = {
            let mut state = self.shared.lock();
            if let Some(session) = session {
                state.closed_sessions.insert(session.clone());
            }
            let ids: Vec<_> = state
                .owners
                .values()
                .filter_map(std::sync::Weak::upgrade)
                .filter(|owner| matches(owner))
                .map(|owner| {
                    owner.closed.store(true, Ordering::SeqCst);
                    owner.id
                })
                .collect();
            let mut senders = Vec::new();
            for entry in state.entries.values_mut() {
                let remove: Vec<_> = entry
                    .waiters
                    .iter()
                    .filter(|(_, w)| ids.contains(&w.owner.id))
                    .map(|(id, _)| *id)
                    .collect();
                for id in remove {
                    if let Some(waiter) = entry.waiters.remove(&id) {
                        senders.push(waiter.sender);
                    }
                }
            }
            let empty: Vec<_> = state
                .entries
                .iter()
                .filter(|(_, e)| e.waiters.is_empty())
                .map(|(id, _)| *id)
                .collect();
            for id in empty {
                state.take(id);
            }
            senders
        };
        for sender in senders {
            let _ = sender.send(PromptOutcome::Cancelled);
        }
    }
}

impl PromptWait {
    /// Only the first waiter reveals the UI for a shared proxy question.
    pub fn is_first_waiter(&self) -> bool {
        self.first_waiter
    }

    /// Returns the unchanged wire ID; internal generations never cross IPC.
    pub fn id(&self) -> &super::PromptId {
        &self.id
    }

    /// Waits once, retaining individual deadlines for shared proxy questions.
    pub async fn wait(mut self) -> Result<PromptOutcome, String> {
        let receiver = self
            .receiver
            .take()
            .ok_or("Prompt receiver was already taken")?;
        match tokio::time::timeout(self.timeout, receiver).await {
            Ok(Ok(outcome)) => Ok(outcome),
            Ok(Err(_)) => Ok(PromptOutcome::ReceiverClosed),
            Err(_) => {
                let entry = {
                    let mut state = self.shared.lock();
                    if state
                        .entries
                        .get(&self.generation)
                        .is_some_and(|e| !e.resolving)
                    {
                        state.take(self.generation)
                    } else {
                        None
                    }
                };
                if let Some(entry) = entry {
                    let expired = entry.request.expired_event;
                    for (id, waiter) in entry.waiters {
                        if id != self.waiter {
                            let _ = waiter.sender.send(PromptOutcome::ReceiverClosed);
                        }
                    }
                    if let Some(event) = expired {
                        let _ = self.shared.events.emit(event);
                    }
                }
                Ok(PromptOutcome::TimedOut)
            }
        }
    }
}

impl Drop for PromptWait {
    fn drop(&mut self) {
        self.shared.remove_waiter(self.generation, self.waiter);
    }
}
