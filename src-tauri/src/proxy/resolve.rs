//! Turns a host's proxy choice into the server its connection dials.

use super::{proxy_secret_key, storage, ProxyChoice, ProxyEndpoint, ProxyServer};
use crate::ssh::secret_store::types::SecretStoreState;
use tauri::AppHandle;
use zeroize::Zeroizing;

/// Which saved server `choice` names, given this device's default proxy id
/// (empty when none). `None` connects directly.
fn chosen_id<'a>(choice: &'a ProxyChoice, default_id: &'a str) -> Option<&'a str> {
    match choice {
        ProxyChoice::Direct => None,
        ProxyChoice::Default => Some(default_id.trim()).filter(|id| !id.is_empty()),
        ProxyChoice::Server { id } => Some(id.as_str()),
    }
}

/// Why a chosen server cannot be found, worded for where the user fixes it.
fn missing_server_error(choice: &ProxyChoice) -> String {
    match choice {
        ProxyChoice::Default => {
            "The default proxy no longer exists. Choose another one in Settings > Proxies."
                .to_string()
        }
        _ => "The proxy this host connects through no longer exists. Choose another one in the host's settings."
            .to_string(),
    }
}

fn endpoint(server: ProxyServer, password: Option<Zeroizing<String>>) -> ProxyEndpoint {
    ProxyEndpoint {
        name: server.name,
        kind: server.kind,
        host: server.host,
        port: server.port,
        password: server.username.as_ref().and(password),
        username: server.username,
    }
}

/// The proxy server a connection with `choice` dials, with its saved
/// password, or `None` to connect directly.
pub fn resolve_proxy(
    app: &AppHandle,
    secret_state: &SecretStoreState,
    choice: &ProxyChoice,
) -> Result<Option<ProxyEndpoint>, String> {
    let default_id = match choice {
        ProxyChoice::Default => crate::config::load_config_file()?.default_proxy_id,
        _ => String::new(),
    };
    let Some(id) = chosen_id(choice, &default_id) else {
        return Ok(None);
    };
    let server = crate::db::read(|connection| storage::get_proxy(connection, id))?
        .ok_or_else(|| missing_server_error(choice))?;
    let password = if server.username.is_some() {
        let key = proxy_secret_key(id);
        let password = secret_state.get_password(app, &key)?;
        if password.is_none() {
            if let Some(reason) = secret_state.unreadable_reason(&[key.as_str()])? {
                return Err(reason.to_string());
            }
        }
        password.map(Zeroizing::new)
    } else {
        None
    };
    Ok(Some(endpoint(server, password)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::ProxyKind;

    #[test]
    fn the_default_choice_follows_the_device_default() {
        assert_eq!(chosen_id(&ProxyChoice::Default, ""), None);
        assert_eq!(chosen_id(&ProxyChoice::Default, "  "), None);
        assert_eq!(chosen_id(&ProxyChoice::Default, "office"), Some("office"));
        assert_eq!(chosen_id(&ProxyChoice::Direct, "office"), None);
        let server = ProxyChoice::Server {
            id: "home".to_string(),
        };
        assert_eq!(chosen_id(&server, "office"), Some("home"));
    }

    #[test]
    fn a_password_is_kept_only_for_a_server_with_a_user() {
        let server = ProxyServer {
            id: "p".to_string(),
            name: "P".to_string(),
            kind: ProxyKind::Http,
            host: "proxy".to_string(),
            port: 8080,
            username: None,
        };
        let password = Some(Zeroizing::new("secret".to_string()));
        assert!(endpoint(server.clone(), password.clone())
            .password
            .is_none());

        let with_user = ProxyServer {
            username: Some("me".to_string()),
            ..server
        };
        let resolved = endpoint(with_user, password);
        assert_eq!(
            resolved.password.as_deref().map(String::as_str),
            Some("secret")
        );
        assert!(!format!("{resolved:?}").contains("secret"));
    }
}
