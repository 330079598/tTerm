//! Proxies that SSH connections reach their first host through: an HTTP
//! proxy (`CONNECT`), a SOCKS5 proxy, or a local command whose input and
//! output carry the connection, as OpenSSH's `ProxyCommand`. The proxies are
//! kept in a list that hosts refer to by id, with one of them as this
//! device's default.
//!
//! `storage` keeps the list, `resolve` turns a host's choice into the proxy
//! to use, and `connect` opens the stream through it: over TCP, or by running
//! the command (`command`) once the user allowed it here (`approval`).

pub(crate) mod approval;
pub(crate) mod command;
pub(crate) mod connect;
mod http_connect;
pub(crate) mod resolve;
mod socks5;
pub(crate) mod storage;

use crate::core::blocking::run_blocking;
use crate::ssh::secret_store::types::{SecretLocation, SecretStoreState};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::{AppHandle, State};
use zeroize::Zeroizing;

/// The protocol a proxy server speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyKind {
    /// An HTTP proxy that opens tunnels with `CONNECT`.
    Http,
    /// A SOCKS5 proxy, which also resolves the target's host name.
    Socks5,
    /// A local command whose standard input and output carry the connection.
    Command,
}

impl ProxyKind {
    /// How messages and names write the kind, e.g. `SOCKS5`.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Http => "HTTP",
            Self::Socks5 => "SOCKS5",
            Self::Command => "command",
        }
    }
}

/// A saved proxy, without its password. A command proxy has only a command;
/// the others have a host and port instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyServer {
    pub id: String,
    pub name: String,
    pub kind: ProxyKind,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    /// Sign in to the proxy as this user; no authentication when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// The command line, with OpenSSH's `%h`, `%p`, `%r` and `%%` tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// A saved proxy server as the settings list shows it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyServerEntry {
    #[serde(flatten)]
    pub server: ProxyServer,
    /// Whether a password is saved for the server's user.
    pub has_password: bool,
}

/// Which proxy a host connects through.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum ProxyChoice {
    /// This device's default proxy, or none when no default is set.
    #[default]
    Default,
    /// No proxy, even when a default is set.
    Direct,
    /// The saved proxy server with this id.
    Server { id: String },
}

impl ProxyChoice {
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Default)
    }
}

/// A proxy to use, with its password.
#[derive(Clone)]
pub struct ProxyEndpoint {
    pub name: String,
    pub kind: ProxyKind,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<Zeroizing<String>>,
    /// The command line of a command proxy, before its tokens are replaced.
    pub command: Option<String>,
}

impl ProxyEndpoint {
    /// How progress messages and errors name the proxy, e.g. `SOCKS5 proxy "Office" (10.0.0.1:1080)`.
    pub fn describe(&self) -> String {
        if self.kind == ProxyKind::Command {
            return format!("proxy command \"{}\"", self.name);
        }
        format!(
            "{} proxy \"{}\" ({}:{})",
            self.kind.label(),
            self.name,
            self.host,
            self.port
        )
    }
}

impl std::fmt::Debug for ProxyEndpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProxyEndpoint")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .field("command", &self.command)
            .finish()
    }
}

/// The key a proxy server's password is saved under.
pub fn proxy_secret_key(id: &str) -> String {
    format!("proxy:{id}")
}

/// Trims the fields and checks that the server can be dialed.
fn normalize_server(mut server: ProxyServer) -> Result<ProxyServer, String> {
    server.id = server.id.trim().to_string();
    server.name = server.name.trim().to_string();
    // An IPv6 address may be typed the way URLs write it.
    server.host = server
        .host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string();
    server.username = server
        .username
        .map(|username| username.trim().to_string())
        .filter(|username| !username.is_empty());
    if server.id.is_empty() {
        return Err("The proxy has no id.".to_string());
    }
    if server.name.is_empty() {
        return Err("Enter a name for the proxy.".to_string());
    }
    if server.kind == ProxyKind::Command {
        return normalize_command(server);
    }
    server.command = None;
    if server.host.is_empty() || server.host.contains(char::is_whitespace) {
        return Err("Enter the proxy's host name or IP address.".to_string());
    }
    if server.port == 0 {
        return Err("Enter the proxy's port, between 1 and 65535.".to_string());
    }
    Ok(server)
}

fn normalize_command(mut server: ProxyServer) -> Result<ProxyServer, String> {
    let command = server
        .command
        .as_deref()
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .ok_or_else(|| "Enter the command to run.".to_string())?;
    if command.contains(['\n', '\r']) {
        return Err("The command must be on one line.".to_string());
    }
    command::expand(
        command,
        &command::CommandTarget {
            host: "host",
            port: 22,
            username: "user",
        },
    )?;
    server.command = Some(command.to_string());
    server.host = String::new();
    server.port = 0;
    server.username = None;
    Ok(server)
}

fn list_entries(secret_state: &SecretStoreState) -> Result<Vec<ProxyServerEntry>, String> {
    let saved: HashSet<String> = secret_state.saved_keys()?.into_iter().collect();
    Ok(crate::db::read(storage::list_proxies)?
        .into_iter()
        .map(|server| ProxyServerEntry {
            has_password: server.username.is_some()
                && saved.contains(&proxy_secret_key(&server.id)),
            server,
        })
        .collect())
}

#[tauri::command]
pub async fn list_proxies(
    secret_state: State<'_, SecretStoreState>,
) -> Result<Vec<ProxyServerEntry>, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || list_entries(&secret_state)).await
}

/// Adds or updates a proxy server. `password` replaces the saved one when
/// given; `clear_password` removes it, as does a server without a user.
#[tauri::command]
pub async fn save_proxy(
    app: AppHandle,
    server: ProxyServer,
    password: Option<String>,
    clear_password: bool,
    secret_state: State<'_, SecretStoreState>,
) -> Result<Vec<ProxyServerEntry>, String> {
    let secret_state = secret_state.inner().clone();
    let password = password.map(Zeroizing::new);
    run_blocking(move || {
        let server = normalize_server(server)?;
        let key = proxy_secret_key(&server.id);
        let new_password = password.as_deref().filter(|value| !value.is_empty());
        match (&server.username, new_password) {
            (Some(_), Some(password)) if !clear_password => {
                let location = secret_state.save_password(&app, &key, password)?;
                if matches!(location, SecretLocation::Memory) {
                    return Err(
                        "Proxy password persistence is unavailable. Enable the app vault or use a supported system credential store."
                            .to_string(),
                    );
                }
            }
            (Some(_), None) if !clear_password => {}
            _ => {
                secret_state.delete_password(&app, &key)?;
            }
        }
        crate::db::write(|transaction| {
            storage::upsert_proxy(transaction, &server)?;
            // A command typed here needs no second confirmation to run.
            match &server.command {
                Some(command) => approval::allow(transaction, command),
                None => Ok(()),
            }
        })?;
        list_entries(&secret_state)
    })
    .await
}

/// Removes a proxy server and its password. Hosts that chose it keep the
/// choice and report the proxy as missing when they connect.
#[tauri::command]
pub async fn delete_proxy(
    app: AppHandle,
    id: String,
    secret_state: State<'_, SecretStoreState>,
) -> Result<Vec<ProxyServerEntry>, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || {
        crate::db::write(|transaction| storage::delete_proxy(transaction, &id))?;
        secret_state.delete_password(&app, &proxy_secret_key(&id))?;
        list_entries(&secret_state)
    })
    .await
}

#[cfg(test)]
mod real_proxy_tests;
#[cfg(test)]
mod tests;
