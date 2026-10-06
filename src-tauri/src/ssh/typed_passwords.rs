//! Passwords typed while a quick connection ([`super::AuthMethod::Auto`])
//! signed in, kept in memory while its tab is open. The tab's SFTP,
//! monitoring and reconnects sign in with them instead of asking again, and
//! saving the tab as a profile can store them. They never reach the frontend.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard};

use zeroize::Zeroizing;

/// One account on one host, as seen from one tab.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Account {
    tab_id: String,
    host: String,
    port: u16,
    username: String,
}

impl Account {
    fn new(tab_id: &str, host: &str, port: u16, username: &str) -> Self {
        Self {
            tab_id: tab_id.to_string(),
            host: host.to_string(),
            port,
            username: username.to_string(),
        }
    }
}

static PASSWORDS: LazyLock<Mutex<HashMap<Account, Zeroizing<String>>>> =
    LazyLock::new(Default::default);

fn passwords() -> MutexGuard<'static, HashMap<Account, Zeroizing<String>>> {
    PASSWORDS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

pub fn remember(tab_id: &str, host: &str, port: u16, username: &str, password: &str) {
    passwords().insert(
        Account::new(tab_id, host, port, username),
        Zeroizing::new(password.to_string()),
    );
}

pub fn get(tab_id: &str, host: &str, port: u16, username: &str) -> Option<Zeroizing<String>> {
    passwords()
        .get(&Account::new(tab_id, host, port, username))
        .cloned()
}

/// Drops a password the server no longer accepts.
pub fn forget(tab_id: &str, host: &str, port: u16, username: &str) {
    passwords().remove(&Account::new(tab_id, host, port, username));
}

/// Whether a password was typed for this account in this tab, for the save
/// dialog to offer storing it.
#[tauri::command]
pub fn has_typed_password(tab_id: String, host: String, port: u16, username: String) -> bool {
    passwords().contains_key(&Account::new(&tab_id, &host, port, &username))
}

/// The tab was closed: nothing it typed is needed anymore.
#[tauri::command]
pub fn forget_typed_passwords(tab_id: String) {
    passwords().retain(|account, _| account.tab_id != tab_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_are_kept_per_tab_and_account() {
        remember("typed-tab-a", "db", 22, "root", "secret");
        assert_eq!(
            get("typed-tab-a", "db", 22, "root")
                .as_deref()
                .map(String::as_str),
            Some("secret")
        );
        assert!(get("typed-tab-b", "db", 22, "root").is_none());
        assert!(get("typed-tab-a", "db", 2222, "root").is_none());
        assert!(has_typed_password(
            "typed-tab-a".into(),
            "db".into(),
            22,
            "root".into()
        ));

        forget_typed_passwords("typed-tab-a".into());
        assert!(get("typed-tab-a", "db", 22, "root").is_none());
    }
}
