//! Asking the user for what authentication needs.
use tterm_core::events::ssh::AuthPromptField;
use tterm_core::events::ssh::AuthPromptPayload;

use super::methods::prompt_payload;
use super::{AuthPrompter, AuthTarget, PROMPT_TIMEOUT};
use crate::ssh::types::SshConnectError;
use tterm_core::events::AppEvent;
use tterm_core::events::ssh::SshEvent;
use zeroize::Zeroizing;

impl AuthPrompter {
    pub fn tab(app: &crate::ssh::context::ConnectionContext, tab_id: &str) -> Self {
        Self::Tab {
            app: app.clone(),
            tab_id: tab_id.to_string(),
            owner: None,
        }
    }

    /// Binds all rounds of this authentication to the connection attempt.
    pub fn with_owner(&self, owner: &tterm_core::prompts::PromptOwner) -> Self {
        match self {
            Self::Tab { app, tab_id, .. } => Self::Scoped {
                broker: app.prompts.clone(),
                window: app.window.clone(),
                tab_id: tab_id.clone(),
                owner: owner.clone(),
            },
            other => other.clone(),
        }
    }

    pub(super) fn ensure_open(&self) -> Result<(), SshConnectError> {
        if matches!(self, Self::Scoped { owner, .. } if owner.is_closed()) {
            return Err(SshConnectError::Permanent(
                "SSH authentication was cancelled".into(),
            ));
        }
        Ok(())
    }

    /// Asks for the account's password on `target` for sudo, which the file
    /// manager's sudo mode needs when none is saved. `retry` says sudo turned
    /// down the one typed before.
    pub async fn ask_sudo_password(
        &self,
        target: &AuthTarget<'_>,
        retry: bool,
    ) -> Result<Zeroizing<String>, String> {
        let mut payload = prompt_payload(
            target,
            "sudo",
            String::new(),
            String::new(),
            vec![AuthPromptField {
                text: "Password".to_string(),
                echo: false,
            }],
        );
        payload.retry = retry;
        let answers = self.ask(target, payload).await.map_err(String::from)?;
        answers
            .into_iter()
            .next()
            .map(Zeroizing::new)
            .ok_or_else(|| "No sudo password was entered".to_string())
    }

    pub(super) async fn ask(
        &self,
        target: &AuthTarget<'_>,
        payload: AuthPromptPayload,
    ) -> Result<Vec<String>, SshConnectError> {
        self.ensure_open()?;
        let subject = if payload.kind == "sudo" {
            "sudo"
        } else {
            target.subject()
        };
        #[cfg(test)]
        let cancelled =
            || SshConnectError::Permanent(format!("{subject} authentication was cancelled"));

        match self {
            Self::Unavailable => Err(SshConnectError::Permanent(format!(
                "{subject} authentication needs input from you ({}), which is not possible for \
                 this kind of connection",
                describe_prompts(&payload)
            ))),
            Self::Tab { app, tab_id, owner } => {
                let pending = &app.prompts;
                let fallback;
                let owner = match owner {
                    Some(owner) => owner,
                    None => {
                        fallback = app
                            .prompt_scope(tab_id)
                            .await
                            .map_err(SshConnectError::Permanent)?;
                        &fallback.owner
                    }
                };
                ask_scoped(
                    pending,
                    owner,
                    tab_id,
                    payload,
                    subject,
                    app.window.as_ref(),
                )
                .await
            }
            Self::Scoped {
                broker,
                owner,
                tab_id,
                window,
            } => ask_scoped(broker, owner, tab_id, payload, subject, window.as_ref()).await,
            #[cfg(test)]
            Self::Scripted(script) => {
                let mut script = script.lock().expect("scripted prompts lock");
                script.asked.push(payload);
                script.answers.pop_front().flatten().ok_or_else(cancelled)
            }
        }
    }
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

async fn ask_scoped(
    pending: &tterm_core::prompts::PromptBroker,
    owner: &tterm_core::prompts::PromptOwner,
    tab_id: &str,
    mut payload: AuthPromptPayload,
    subject: &str,
    window: &dyn tterm_platform::window::MainWindow,
) -> Result<Vec<String>, SshConnectError> {
    let request_id = uuid::Uuid::new_v4().to_string();
    payload.request_id = request_id.clone();
    let wait = pending
        .begin(
            tterm_core::prompts::PromptRequest {
                id: tterm_core::prompts::PromptId(request_id.clone()),
                kind: tterm_core::prompts::PromptKind::Auth,
                event: AppEvent::Ssh(SshEvent::AuthPrompt {
                    tab_id: tab_id.to_string(),
                    payload,
                }),
                expired_event: Some(AppEvent::Ssh(SshEvent::AuthPromptExpired {
                    tab_id: tab_id.to_string(),
                    request_id,
                })),
            },
            tterm_core::prompts::PromptOptions {
                owner: owner.clone(),
                timeout: PROMPT_TIMEOUT,
            },
        )
        .map_err(SshConnectError::Permanent)?;
    let _ = window.reveal_for_prompt();
    match wait.wait().await.map_err(SshConnectError::Permanent)? {
        tterm_core::prompts::PromptOutcome::Answered(tterm_core::prompts::PromptReply::Auth(
            Some(answers),
        )) => Ok(answers),
        tterm_core::prompts::PromptOutcome::TimedOut => Err(SshConnectError::Permanent(format!(
            "{subject} authentication timed out waiting for your input"
        ))),
        _ => Err(SshConnectError::Permanent(format!(
            "{subject} authentication was cancelled"
        ))),
    }
}
