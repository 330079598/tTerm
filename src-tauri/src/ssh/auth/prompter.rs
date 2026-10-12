//! Asking the user for what authentication needs.

use super::{AuthPromptMap, AuthPromptPayload, AuthPrompter, AuthTarget, PROMPT_TIMEOUT};
use crate::ssh::types::SshConnectError;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tokio::sync::oneshot;

impl AuthPrompter {
    pub fn tab(app: &tauri::AppHandle, tab_id: &str) -> Self {
        Self::Tab {
            app: app.clone(),
            tab_id: tab_id.to_string(),
        }
    }

    pub(super) async fn ask(
        &self,
        target: &AuthTarget<'_>,
        mut payload: AuthPromptPayload,
    ) -> Result<Vec<String>, SshConnectError> {
        let subject = target.subject();
        let cancelled =
            || SshConnectError::Permanent(format!("{subject} authentication was cancelled"));

        match self {
            Self::Unavailable => Err(SshConnectError::Permanent(format!(
                "{subject} authentication needs input from you ({}), which is not possible for \
                 this kind of connection",
                describe_prompts(&payload)
            ))),
            Self::Tab { app, tab_id } => {
                let Some(pending) = app.try_state::<AuthPromptMap>() else {
                    return Err(SshConnectError::Permanent(format!(
                        "{subject} authentication prompts are unavailable"
                    )));
                };
                let pending = pending.inner().clone();
                let request_id = uuid::Uuid::new_v4().to_string();
                payload.request_id = request_id.clone();
                let (tx, rx) = oneshot::channel();
                lock(&pending).insert(request_id.clone(), tx);

                let event_name = format!("ssh-auth-prompt-{tab_id}");
                let _ = app.emit_to(tauri::EventTarget::any(), &event_name, payload);
                crate::background::reveal_for_prompt(app);

                match tokio::time::timeout(PROMPT_TIMEOUT, rx).await {
                    Ok(Ok(Some(answers))) => Ok(answers),
                    Ok(_) => Err(cancelled()),
                    Err(_) => {
                        lock(&pending).remove(&request_id);
                        // Tell the frontend to close the dialog it is still showing.
                        let _ = app.emit_to(
                            tauri::EventTarget::any(),
                            &format!("ssh-auth-prompt-expired-{tab_id}"),
                            request_id,
                        );
                        Err(SshConnectError::Permanent(format!(
                            "{subject} authentication timed out waiting for your input"
                        )))
                    }
                }
            }
            #[cfg(test)]
            Self::Scripted(script) => {
                let mut script = script.lock().expect("scripted prompts lock");
                script.asked.push(payload);
                script.answers.pop_front().flatten().ok_or_else(cancelled)
            }
        }
    }
}

pub(super) fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

pub(super) fn describe_prompts(payload: &AuthPromptPayload) -> String {
    let texts = payload
        .prompts
        .iter()
        .map(|prompt| prompt.text.trim().trim_end_matches(':').trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    if texts.is_empty() {
        payload.kind.to_string()
    } else {
        texts.join(", ")
    }
}
