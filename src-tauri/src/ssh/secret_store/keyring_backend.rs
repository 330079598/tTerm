use super::types::SERVICE_NAME;

pub(crate) fn secret_key_name(profile_id: &str, kind: &str) -> String {
    format!("{}::{}", kind, profile_id)
}

pub(crate) fn read_keyring_secret(
    profile_id: &str,
    kind: &str,
) -> Result<Option<String>, keyring::Error> {
    let account = secret_key_name(profile_id, kind);
    let entry = keyring::Entry::new(SERVICE_NAME, &account)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err),
    }
}

pub(crate) fn write_keyring_secret(
    profile_id: &str,
    kind: &str,
    value: &str,
) -> Result<(), String> {
    let account = secret_key_name(profile_id, kind);
    let entry = keyring::Entry::new(SERVICE_NAME, &account)
        .map_err(|e| format!("Failed to open keyring entry: {}", e))?;
    entry
        .set_password(value)
        .map_err(|e| format!("Failed to write keyring secret: {}", e))
}

pub(crate) fn delete_keyring_secret(profile_id: &str, kind: &str) -> Result<bool, String> {
    let account = secret_key_name(profile_id, kind);
    let entry = keyring::Entry::new(SERVICE_NAME, &account)
        .map_err(|e| format!("Failed to open keyring entry: {}", e))?;
    match entry.delete_credential() {
        Ok(()) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(err) => Err(format!("Failed to delete keyring secret: {}", err)),
    }
}
