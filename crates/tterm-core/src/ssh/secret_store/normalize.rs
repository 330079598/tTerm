//! Normalization of partial status payloads for display, not authorization.

use super::types::{SecretBackendStatus, SecretStorageMode};
use serde_json::Value;

/// Applies the UI defaults to status fields missing from an older payload.
pub fn normalize_secret_status(status: &Value) -> SecretBackendStatus {
    let text = |field: &str| status.get(field).and_then(Value::as_str);
    let flag = |field: &str| status.get(field).and_then(Value::as_bool).unwrap_or(false);
    let method = text("verificationMethod")
        .filter(|value| matches!(*value, "system" | "masterPassword"))
        .unwrap_or("none");
    SecretBackendStatus {
        storage_mode: SecretStorageMode::from_config_value(text("storageMode").unwrap_or_default())
            .as_str()
            .to_owned(),
        keyring_available: flag("keyringAvailable"),
        unlocked: flag("unlocked"),
        has_master_password: flag("hasMasterPassword"),
        persistence_available: flag("persistenceAvailable"),
        migration_pending: flag("migrationPending"),
        verification_method: method.to_owned(),
        message: text("message").map(str::to_owned),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn partial_status_uses_typed_defaults() {
        for value in [
            Value::Null,
            json!({}),
            json!({"unlocked":"yes", "message":42, "verificationMethod":"password"}),
        ] {
            let status = normalize_secret_status(&value);
            assert_eq!(status.storage_mode, "system");
            assert_eq!(status.verification_method, "none");
            assert_eq!(status.message, None);
            assert!(
                !status.unlocked
                    && !status.has_master_password
                    && !status.keyring_available
                    && !status.migration_pending
                    && !status.persistence_available
            );
        }
    }

    #[test]
    fn legacy_mode_and_supported_methods_match_the_ui() {
        for method in ["system", "masterPassword"] {
            let status = normalize_secret_status(
                &json!({"storageMode":"vault", "verificationMethod":method, "message":"hi", "keyringAvailable":true, "unlocked":true, "hasMasterPassword":true, "persistenceAvailable":true, "migrationPending":true}),
            );
            assert_eq!(status.storage_mode, "password");
            assert_eq!(status.verification_method, method);
            assert_eq!(status.message.as_deref(), Some("hi"));
            assert!(
                status.unlocked
                    && status.has_master_password
                    && status.keyring_available
                    && status.migration_pending
                    && status.persistence_available
            );
        }
    }
}
