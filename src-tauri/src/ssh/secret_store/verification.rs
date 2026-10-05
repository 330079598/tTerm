//! Confirms that the person at the keyboard is the user before saved
//! passwords leave the app: shown in Settings, exported in a backup, or sent
//! to a newly chosen WebDAV server. The check runs here rather than in the
//! web view, so a script there cannot skip it.
//!
//! Windows Hello or Touch ID is used when the OS offers it, else the master
//! password. Without either (e.g. Linux in `system` mode with no recovery password)
//! there is nothing to check against and the operations run as before.

use super::types::{SecretStorageMode, SecretStoreRuntime};
use super::{open_with_password, os_verifier, store, SecretStoreState};
use serde::Deserialize;
use std::time::{Duration, Instant};
use tauri::AppHandle;

/// Prefix of the error asking the UI to verify and retry. The method
/// (`system` or `masterPassword`) follows it.
pub const VERIFICATION_REQUIRED: &str = "USER_VERIFICATION_REQUIRED:";
/// Showing several passwords in a row needs one verification.
const REVEAL_WINDOW: Duration = Duration::from_secs(60);
/// How long a verification waits for the operation it was made for.
const SENSITIVE_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VerificationPurpose {
    /// Showing saved passwords in Settings.
    Reveal,
    /// Sending saved passwords somewhere new: a backup file, another WebDAV
    /// server, or dropping the recovery password that guards them.
    Sensitive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VerificationMethod {
    System,
    MasterPassword,
    None,
}

impl VerificationMethod {
    /// A master password chosen as the storage mode is what the user expects
    /// to type; otherwise the OS check is preferred when there is one.
    pub(crate) fn choose(
        mode: SecretStorageMode,
        os_verifier_available: bool,
        has_master_password: bool,
    ) -> Self {
        if mode != SecretStorageMode::Password && os_verifier_available {
            Self::System
        } else if has_master_password {
            Self::MasterPassword
        } else {
            Self::None
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::MasterPassword => "masterPassword",
            Self::None => "none",
        }
    }
}

impl SecretStoreRuntime {
    /// Whether a recent verification covers `purpose`. A sensitive one is
    /// used up by the first check, whatever its outcome.
    fn take_grant(&mut self, purpose: VerificationPurpose, now: Instant) -> bool {
        match purpose {
            VerificationPurpose::Reveal => {
                self.reveal_verified_until.is_some_and(|until| now < until)
            }
            VerificationPurpose::Sensitive => self
                .sensitive_verified_until
                .take()
                .is_some_and(|until| now < until),
        }
    }

    fn grant(&mut self, purpose: VerificationPurpose, now: Instant) {
        match purpose {
            VerificationPurpose::Reveal => self.reveal_verified_until = Some(now + REVEAL_WINDOW),
            VerificationPurpose::Sensitive => {
                self.sensitive_verified_until = Some(now + SENSITIVE_WINDOW)
            }
        }
    }

    pub(super) fn clear_grants(&mut self) {
        self.reveal_verified_until = None;
        self.sensitive_verified_until = None;
    }
}

impl SecretStoreState {
    /// Checked once per run; a failed OS prompt checks again.
    pub(super) fn os_verifier_available(&self) -> Result<bool, String> {
        if let Some(value) = self.runtime()?.os_verifier_available {
            return Ok(value);
        }
        let available = os_verifier::available();
        self.runtime()?.os_verifier_available = Some(available);
        Ok(available)
    }

    pub(crate) fn verification_method(&self) -> Result<VerificationMethod, String> {
        let has_master_password =
            crate::db::read(|connection| store::has_wrap(connection, store::WRAP_PASSWORD))?;
        Ok(VerificationMethod::choose(
            Self::mode()?,
            self.os_verifier_available()?,
            has_master_password,
        ))
    }

    /// Call right before the operation, after its own checks, so a rejected
    /// request does not use up the verification.
    pub(crate) fn require_verification(&self, purpose: VerificationPurpose) -> Result<(), String> {
        if self.runtime()?.take_grant(purpose, Instant::now()) {
            return Ok(());
        }
        match self.verification_method()? {
            VerificationMethod::None => Ok(()),
            method => Err(format!("{VERIFICATION_REQUIRED}{}", method.as_str())),
        }
    }

    /// Asks the OS (Windows Hello, Touch ID) or checks `password` against the
    /// master password, then allows `purpose` for a short while. Blocks.
    pub fn verify_user(
        &self,
        app: &AppHandle,
        purpose: VerificationPurpose,
        password: Option<&str>,
    ) -> Result<(), String> {
        match self.verification_method()? {
            VerificationMethod::System => {
                if let Err(error) = os_verifier::verify(app, prompt_message(purpose)) {
                    if error != os_verifier::CANCELED {
                        // The OS check may have been turned off since.
                        self.runtime()?.os_verifier_available = None;
                    }
                    return Err(error);
                }
            }
            VerificationMethod::MasterPassword => {
                let password = password
                    .filter(|password| !password.is_empty())
                    .ok_or_else(|| "Enter the master password.".to_string())?;
                if open_with_password(crate::db::get()?, password)?.is_none() {
                    return Err("No master password is set.".to_string());
                }
            }
            VerificationMethod::None => {}
        }
        self.runtime()?.grant(purpose, Instant::now());
        Ok(())
    }
}

/// Shown by the OS in its own prompt, in the app's language. macOS words it
/// as "tTerm is trying to <reason>".
fn prompt_message(purpose: VerificationPurpose) -> &'static str {
    let chinese = crate::config::load_config_file().is_ok_and(|config| config.language == "zh");
    if cfg!(target_os = "macos") {
        return match (purpose, chinese) {
            (VerificationPurpose::Reveal, true) => "显示已保存的密码",
            (VerificationPurpose::Reveal, false) => "show a saved password",
            (VerificationPurpose::Sensitive, true) => "访问已保存的密码",
            (VerificationPurpose::Sensitive, false) => "access saved passwords",
        };
    }
    match (purpose, chinese) {
        (VerificationPurpose::Reveal, true) => "tTerm 需要验证你的身份才能显示已保存的密码。",
        (VerificationPurpose::Reveal, false) => {
            "tTerm wants to verify it's you before showing a saved password."
        }
        (VerificationPurpose::Sensitive, true) => "tTerm 需要验证你的身份才能继续。",
        (VerificationPurpose::Sensitive, false) => {
            "tTerm wants to verify it's you before continuing."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_method_follows_the_storage_mode() {
        use SecretStorageMode::*;
        use VerificationMethod as M;
        assert_eq!(M::choose(System, true, true), M::System);
        assert_eq!(M::choose(System, false, true), M::MasterPassword);
        assert_eq!(M::choose(System, false, false), M::None);
        assert_eq!(M::choose(Password, true, true), M::MasterPassword);
        assert_eq!(M::choose(Memory, true, false), M::System);
    }

    #[test]
    fn a_reveal_lasts_a_while_and_a_sensitive_grant_is_used_once() {
        let mut runtime = SecretStoreRuntime::default();
        let now = Instant::now();
        assert!(!runtime.take_grant(VerificationPurpose::Reveal, now));

        runtime.grant(VerificationPurpose::Reveal, now);
        assert!(runtime.take_grant(VerificationPurpose::Reveal, now));
        assert!(runtime.take_grant(VerificationPurpose::Reveal, now));
        assert!(!runtime.take_grant(VerificationPurpose::Reveal, now + REVEAL_WINDOW));
        // Showing passwords does not open the door to exporting them.
        assert!(!runtime.take_grant(VerificationPurpose::Sensitive, now));

        runtime.grant(VerificationPurpose::Sensitive, now);
        assert!(runtime.take_grant(VerificationPurpose::Sensitive, now));
        assert!(!runtime.take_grant(VerificationPurpose::Sensitive, now));

        runtime.grant(VerificationPurpose::Sensitive, now);
        assert!(!runtime.take_grant(VerificationPurpose::Sensitive, now + SENSITIVE_WINDOW));

        runtime.grant(VerificationPurpose::Reveal, now);
        runtime.grant(VerificationPurpose::Sensitive, now);
        runtime.clear_grants();
        assert!(!runtime.take_grant(VerificationPurpose::Reveal, now));
        assert!(!runtime.take_grant(VerificationPurpose::Sensitive, now));
    }
}
