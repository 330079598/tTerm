//! Items of the "tterm" service in the OS credential store. Since saved
//! passwords moved into the database, only the key that unwraps the data key
//! is kept here; the other accounts are read once when migrating.

use std::sync::OnceLock;
use zeroize::Zeroizing;

/// The installed app's identifier, which keeps the original service name.
const APP_IDENTIFIER: &str = "com.stone.tTerm";
const SERVICE_NAME: &str = "tterm";
static SERVICE: OnceLock<String> = OnceLock::new();

/// Holds the key that unwraps the database's data key.
pub(crate) const DATA_KEY_ACCOUNT: &str = "key::__secret_data_key__";
/// Hybrid mode's vault master password, from before the database.
pub(crate) const LEGACY_VAULT_MASTER_ACCOUNT: &str = "master::__vault_master__";
/// Prefix of the per-password accounts written before the database.
pub(crate) const LEGACY_PASSWORD_PREFIX: &str = "password::";
const PROBE_ACCOUNT: &str = "__probe__";
const PROBE_SECRET: &str = "tterm-keyring-probe";

/// The service for an app identifier. Builds with another identifier, such
/// as `pnpm tauri:dev`, keep their own items: they have their own database,
/// and sharing the data key item would let each one replace the other's.
fn service_for(identifier: &str) -> String {
    if identifier == APP_IDENTIFIER {
        SERVICE_NAME.to_string()
    } else {
        format!("{SERVICE_NAME} ({identifier})")
    }
}

/// Picks the service for this app before the credential store is first used.
pub(crate) fn init_service(identifier: &str) {
    let _ = SERVICE.set(service_for(identifier));
}

fn service() -> &'static str {
    SERVICE.get().map_or(SERVICE_NAME, String::as_str)
}

fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(service(), account)
        .map_err(|e| format!("Failed to open system credential store entry: {e}"))
}

pub(crate) fn read(account: &str) -> Result<Option<Zeroizing<String>>, String> {
    match entry(account)?.get_password() {
        Ok(value) => Ok(Some(Zeroizing::new(value))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(format!("Failed to read system credential store: {err}")),
    }
}

pub(crate) fn write(account: &str, value: &str) -> Result<(), String> {
    entry(account)?
        .set_password(value)
        .map_err(|e| format!("Failed to write system credential store: {e}"))
}

pub(crate) fn delete(account: &str) -> Result<bool, String> {
    match entry(account)?.delete_credential() {
        Ok(()) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(err) => Err(format!(
            "Failed to delete system credential store entry: {err}"
        )),
    }
}

pub(crate) fn probe() -> bool {
    let Ok(entry) = keyring::Entry::new(service(), PROBE_ACCOUNT) else {
        return false;
    };
    if entry.set_password(PROBE_SECRET).is_err() {
        return false;
    }
    let ok = matches!(entry.get_password(), Ok(value) if value == PROBE_SECRET);
    let _ = entry.delete_credential();
    ok
}

/// Every account name stored under the service, when the platform can list
/// them without reading their secrets (so without any unlock prompt).
#[cfg(target_os = "macos")]
pub(crate) fn list_accounts() -> Option<Vec<String>> {
    use security_framework::item::{ItemClass, ItemSearchOptions, Limit};

    const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;
    match ItemSearchOptions::new()
        .class(ItemClass::generic_password())
        .service(service())
        .load_attributes(true)
        .limit(Limit::All)
        .search()
    {
        Ok(results) => Some(
            results
                .iter()
                .filter_map(|result| result.simplify_dict()?.remove("acct"))
                .collect(),
        ),
        Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Some(Vec::new()),
        Err(error) => {
            eprintln!("Failed to list system credential store entries: {error}");
            None
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn list_accounts() -> Option<Vec<String>> {
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_the_installed_app_uses_the_original_service() {
        assert_eq!(super::service_for("com.stone.tTerm"), "tterm");
        assert_eq!(
            super::service_for("com.stone.tTerm.dev"),
            "tterm (com.stone.tTerm.dev)"
        );
    }

    /// Lists the real credential store's account names (no secrets are read,
    /// so no unlock prompt): `cargo test list_real_accounts -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn list_real_accounts() {
        println!("{:#?}", super::list_accounts());
    }
}
