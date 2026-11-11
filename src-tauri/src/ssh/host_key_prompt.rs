//! Keeps an ended prompt distinct from an explicit refusal through russh errors.
use super::types::SshConnectError;
use std::sync::Mutex;
use tterm_core::prompts::{PromptOutcome, PromptReply, PromptWait};

pub(crate) fn failure(slot: &Mutex<Option<SshConnectError>>) -> Option<SshConnectError> {
    slot.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

pub(crate) async fn resolve(
    wait: Result<PromptWait, String>,
    slot: &Mutex<Option<SshConnectError>>,
) -> Result<(), SshConnectError> {
    let outcome = match wait {
        Ok(wait) => wait.wait().await,
        Err(error) => Err(error),
    };
    let result = classify(outcome);
    *slot.lock().unwrap_or_else(|p| p.into_inner()) = result.as_ref().err().cloned();
    result
}

fn classify(outcome: Result<PromptOutcome, String>) -> Result<(), SshConnectError> {
    match outcome {
        Ok(PromptOutcome::Answered(PromptReply::HostKey(true))) => Ok(()),
        Ok(PromptOutcome::Answered(PromptReply::HostKey(false))) => {
            Err(SshConnectError::HostKeyRejected)
        }
        Ok(PromptOutcome::TimedOut) => Err(SshConnectError::Permanent(
            "SSH host fingerprint confirmation timed out".into(),
        )),
        _ => Err(SshConnectError::Cancelled(
            "SSH host fingerprint confirmation was cancelled".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_or_cancelled_host_prompt_is_never_a_user_refusal() {
        for outcome in [
            Ok(PromptOutcome::Cancelled),
            Ok(PromptOutcome::ReceiverClosed),
            Err("receiver dropped".into()),
        ] {
            let error = classify(outcome).unwrap_err();
            assert!(matches!(error, SshConnectError::Cancelled(_)));
            assert!(!error.is_host_key_rejected());
            assert!(!error.to_string().contains("rejected by user"));
            assert!(matches!(
                error.with_prefix("Jump: "),
                SshConnectError::Cancelled(_)
            ));
        }
        assert!(
            classify(Ok(PromptOutcome::Answered(PromptReply::HostKey(false))))
                .unwrap_err()
                .is_host_key_rejected()
        );
    }
}
