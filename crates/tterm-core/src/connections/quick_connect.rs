//! Parsing what the new-tab launcher accepts and building a quick connection
//! from it, from `src/lib/quickConnect.ts`.
//!
//! The WebView owns storage. These functions take stored text or explicit
//! values and return the models the launcher and tab bar use.

use crate::js::{is_js_whitespace, trim_js};
use crate::profiles::types::{SavedJumpHost, SavedProfile};
use crate::workspace::tab::{Tab, TabType};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The port `ssh` uses when a quick connection does not name one.
pub const DEFAULT_SSH_PORT: u16 = 22;

/// How many quick connections the launcher remembers.
pub const QUICK_CONNECT_RECENTS_LIMIT: usize = 8;

/// One `[user@]host[:port]` endpoint, as typed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickConnectEndpoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    pub host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

/// What was typed into the launcher when it reads as a host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickConnectTarget {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    pub host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jump_host: Option<QuickConnectEndpoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key_path: Option<String>,
}

/// A `Host` entry of `~/.ssh/config`, as the backend's import preview lists it.
#[derive(Debug, Clone)]
pub struct SshConfigHost {
    pub host_pattern: String,
    pub name: String,
    pub host: Option<String>,
    pub port: u16,
    pub username: Option<String>,
    pub private_key_path: Option<String>,
    pub agent_forward: bool,
    pub keepalive_interval_secs: u32,
    pub keepalive_count_max: u32,
    pub jump_hosts: Vec<SavedJumpHost>,
    pub skipped: bool,
}

/// A jump host in the model a quick connection's tab carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JumpHostConnection {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key_path: Option<String>,
}

/// A connection to open: everything a quick connect tab needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickConnection {
    pub title: String,
    pub username: String,
    pub host: String,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_forward: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keepalive_interval_secs: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keepalive_count_max: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jump_hosts: Option<Vec<JumpHostConnection>>,
}

/// A quick connection that reached a shell, offered again in the launcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentQuickConnection {
    #[serde(flatten)]
    pub connection: QuickConnection,
    pub last_used_at: i64,
}

/// The automatic authentication method that got a quick connection in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshAuthUsed {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key_path: Option<String>,
    pub reusable: bool,
}

fn parse_port(value: &str) -> Option<u16> {
    if value.is_empty() || value.len() > 5 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let port = value.parse::<u16>().ok()?;
    (1..=65535).contains(&port).then_some(port)
}

fn is_hostname(host: &str) -> bool {
    let bytes = host.as_bytes();
    let Some(first) = bytes.first() else {
        return false;
    };
    let Some(last) = bytes.last() else {
        return false;
    };
    let edge = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    edge(*first)
        && edge(*last)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_ipv6_text(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || matches!(byte, b'.' | b':' | b'%'))
}

fn parse_bracketed_endpoint(rest: &str) -> Option<QuickConnectEndpoint> {
    if !rest.starts_with('[') {
        return None;
    }
    let close = rest.find(']')?;
    let host = &rest[1..close];
    if !is_ipv6_text(host) {
        return None;
    }
    let suffix = &rest[close + 1..];
    if suffix.is_empty() {
        return Some(QuickConnectEndpoint {
            username: None,
            host: host.to_owned(),
            port: None,
        });
    }
    let port = suffix.strip_prefix(':')?;
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(QuickConnectEndpoint {
        username: None,
        host: host.to_owned(),
        port: Some(parse_port(port)?),
    })
}

/// Reads `[user@]host[:port]`, with an IPv6 address in brackets when a port
/// follows.
pub fn parse_endpoint(text: &str) -> Option<QuickConnectEndpoint> {
    let (username, rest) = match text.rfind('@') {
        Some(at) => {
            let username = &text[..at];
            if username.is_empty()
                || username
                    .chars()
                    .any(|character| is_js_whitespace(character) || matches!(character, ':' | '@'))
            {
                return None;
            }
            (Some(username.to_owned()), &text[at + 1..])
        }
        None => (None, text),
    };

    let mut endpoint = if rest.starts_with('[') {
        parse_bracketed_endpoint(rest)?
    } else {
        let colons = rest.bytes().filter(|byte| *byte == b':').count();
        if colons == 1 {
            let (host, port) = rest.split_once(':')?;
            QuickConnectEndpoint {
                username: None,
                host: host.to_owned(),
                port: Some(parse_port(port)?),
            }
        } else {
            if colons > 0 && !is_ipv6_text(rest) {
                return None;
            }
            QuickConnectEndpoint {
                username: None,
                host: rest.to_owned(),
                port: None,
            }
        }
    };
    if !endpoint.host.contains(':') && !is_hostname(&endpoint.host) {
        return None;
    }
    endpoint.username = username;
    Some(endpoint)
}

fn endpoint_from_token(token: &str) -> Option<QuickConnectEndpoint> {
    parse_endpoint(token)
}

fn option_value<'a>(
    token: &'a str,
    tokens: &'a [String],
    index: &mut usize,
) -> Option<(&'a str, &'a str)> {
    let rest = token.get(1..)?;
    let option = rest.get(..1)?;
    if !matches!(option, "p" | "l" | "i" | "J") {
        return None;
    }
    let attached = &rest[1..];
    if !attached.is_empty() {
        return Some((option, attached));
    }
    *index += 1;
    tokens.get(*index).map(|value| (option, value.as_str()))
}

/// Reads what was typed as a host, or as an `ssh` command line with `-p`,
/// `-l`, `-i` and `-J`.
pub fn parse_quick_connect_input(input: &str) -> Option<QuickConnectTarget> {
    let mut tokens: Vec<String> = trim_js(input)
        .split(is_js_whitespace)
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect();
    if tokens.first().is_some_and(|token| token == "ssh") {
        tokens.remove(0);
    }
    if tokens.is_empty() {
        return None;
    }

    let mut endpoint = None;
    let mut port = None;
    let mut username = None;
    let mut jump_host = None;
    let mut private_key_path = None;
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if let Some((option, value)) = option_value(token, &tokens, &mut index) {
            if value.is_empty() {
                return None;
            }
            match option {
                "p" => port = Some(parse_port(value)?),
                "l" => username = Some(value.to_owned()),
                "i" => private_key_path = Some(value.to_owned()),
                "J" => {
                    if value.contains(',') {
                        return None;
                    }
                    jump_host = Some(parse_endpoint(value)?);
                }
                _ => return None,
            }
        } else if token.starts_with('-') || endpoint.is_some() {
            return None;
        } else {
            endpoint = Some(endpoint_from_token(token)?);
        }
        index += 1;
    }

    let endpoint = endpoint?;
    Some(QuickConnectTarget {
        username: endpoint.username.or(username),
        host: endpoint.host,
        port: port.or(endpoint.port),
        jump_host,
        private_key_path,
    })
}

/// Shows an endpoint the way it is typed.
pub fn format_endpoint(endpoint: &QuickConnectEndpoint) -> String {
    let host = if endpoint.host.contains(':')
        && endpoint.port.is_some_and(|port| port != DEFAULT_SSH_PORT)
    {
        format!("[{}]", endpoint.host)
    } else {
        endpoint.host.clone()
    };
    let user = endpoint
        .username
        .as_ref()
        .map(|username| format!("{username}@"))
        .unwrap_or_default();
    let port = endpoint
        .port
        .filter(|port| *port != DEFAULT_SSH_PORT)
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    format!("{user}{host}{port}")
}

/// The config entry a typed host names, if it is an alias there.
pub fn find_ssh_config_host<'a>(
    hosts: &'a [SshConfigHost],
    alias: &str,
) -> Option<&'a SshConfigHost> {
    hosts
        .iter()
        .find(|host| !host.skipped && host.host_pattern == alias)
}

/// The host to ask the config's wildcard and `Match` blocks about.
pub fn config_host_to_look_up(input: &str, hosts: &[SshConfigHost]) -> Option<String> {
    let target = parse_quick_connect_input(trim_js(input))?;
    find_ssh_config_host(hosts, &target.host)
        .is_none()
        .then_some(target.host)
}

fn jump_hosts_from_config(jump_hosts: &[SavedJumpHost]) -> Vec<JumpHostConnection> {
    jump_hosts
        .iter()
        .map(|jump| JumpHostConnection {
            host: jump.host.clone(),
            port: jump.port,
            username: jump.username.clone(),
            auth_method: "auto".to_owned(),
            private_key_path: jump.private_key_path.clone(),
        })
        .collect()
}

/// Fills a typed target in from `~/.ssh/config`, as `ssh` does.
pub fn resolve_quick_connect_target(
    target: &QuickConnectTarget,
    config_hosts: &[SshConfigHost],
) -> Option<QuickConnection> {
    let config = find_ssh_config_host(config_hosts, &target.host);
    let username = target
        .username
        .clone()
        .or_else(|| config.and_then(|config| config.username.clone()))?;
    let host = config
        .and_then(|config| config.host.clone())
        .unwrap_or_else(|| target.host.clone());
    let port = target
        .port
        .or_else(|| config.map(|config| config.port))
        .unwrap_or(DEFAULT_SSH_PORT);
    let jump_username = target
        .jump_host
        .as_ref()
        .map(|jump| jump.username.clone().unwrap_or_else(|| username.clone()));
    let title = match config {
        Some(config) if target.port.is_none() => format!("{username}@{}", config.name),
        _ => format_endpoint(&QuickConnectEndpoint {
            username: Some(username.clone()),
            host: target.host.clone(),
            port: target.port,
        }),
    };
    let jump_hosts = match target.jump_host.as_ref().zip(jump_username) {
        Some((jump, jump_username)) => Some(vec![JumpHostConnection {
            host: jump.host.clone(),
            port: jump.port.unwrap_or(DEFAULT_SSH_PORT),
            username: jump_username,
            auth_method: "auto".to_owned(),
            private_key_path: None,
        }]),
        None => config
            .filter(|config| !config.jump_hosts.is_empty())
            .map(|config| jump_hosts_from_config(&config.jump_hosts)),
    };
    Some(QuickConnection {
        title,
        username,
        host,
        port,
        private_key_path: target
            .private_key_path
            .clone()
            .or_else(|| config.and_then(|config| config.private_key_path.clone())),
        agent_forward: config.map(|config| config.agent_forward),
        keepalive_interval_secs: config.map(|config| config.keepalive_interval_secs),
        keepalive_count_max: config.map(|config| config.keepalive_count_max),
        jump_hosts,
    })
}

/// The connection of an `~/.ssh/config` entry; `None` without a user name.
pub fn quick_connection_from_config(config: &SshConfigHost) -> Option<QuickConnection> {
    resolve_quick_connect_target(
        &QuickConnectTarget {
            username: None,
            host: config.host_pattern.clone(),
            port: None,
            jump_host: None,
            private_key_path: None,
        },
        std::slice::from_ref(config),
    )
}

fn insert_serialized<T: Serialize + ?Sized>(object: &mut Map<String, Value>, key: &str, value: &T) {
    if let Ok(value) = serde_json::to_value(value) {
        object.insert(key.to_owned(), value);
    }
}

/// A tab for a quick connection: no saved profile, with the method found while
/// connecting.
pub fn build_quick_connect_tab(connection: &QuickConnection) -> Tab {
    let mut tab = Tab::new("", connection.title.clone(), TabType::Ssh);
    tab.set_saved_field("isModified", Some(Value::Bool(false)));
    tab.set_saved_field("quickConnect", Some(Value::Bool(true)));
    let mut saved = Map::new();
    insert_serialized(&mut saved, "type", &TabType::Ssh);
    insert_serialized(&mut saved, "profileName", &connection.title);
    insert_serialized(&mut saved, "host", &connection.host);
    insert_serialized(&mut saved, "port", &connection.port);
    insert_serialized(&mut saved, "username", &connection.username);
    insert_serialized(&mut saved, "authMethod", "auto");
    insert_serialized(
        &mut saved,
        "agentForward",
        &connection.agent_forward.unwrap_or(false),
    );
    if let Some(value) = &connection.private_key_path {
        insert_serialized(&mut saved, "privateKeyPath", value);
    }
    if let Some(value) = connection.keepalive_interval_secs {
        insert_serialized(&mut saved, "keepaliveIntervalSecs", &value);
    }
    if let Some(value) = connection.keepalive_count_max {
        insert_serialized(&mut saved, "keepaliveCountMax", &value);
    }
    if let Some(value) = &connection.jump_hosts {
        insert_serialized(&mut saved, "jumpHosts", value);
    }
    tab.set_saved_field("connection", Some(Value::Object(saved)));
    tab
}

fn connection_field<T: DeserializeOwned>(tab: &Tab, key: &str) -> Option<T> {
    tab.connection
        .as_ref()?
        .saved_field(key)
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

/// The quick connection a tab was opened with, to offer again in the launcher.
pub fn quick_connection_from_tab(tab: &Tab) -> Option<QuickConnection> {
    let host = connection_field::<String>(tab, "host")?;
    let username = connection_field::<String>(tab, "username")?;
    Some(QuickConnection {
        title: tab.title.clone(),
        username,
        host,
        port: connection_field(tab, "port").unwrap_or(DEFAULT_SSH_PORT),
        private_key_path: connection_field(tab, "privateKeyPath"),
        agent_forward: connection_field(tab, "agentForward"),
        keepalive_interval_secs: connection_field(tab, "keepaliveIntervalSecs"),
        keepalive_count_max: connection_field(tab, "keepaliveCountMax"),
        jump_hosts: connection_field(tab, "jumpHosts"),
    })
}

fn recent_key(connection: &QuickConnection) -> String {
    let jumps = connection
        .jump_hosts
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|jump| format!("{}@{}:{}", jump.username, jump.host, jump.port))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{}@{}:{}|{}",
        connection.username, connection.host, connection.port, jumps
    )
}

/// Moves a repeated connection to the front and keeps the newest eight.
pub fn add_recent_quick_connection(
    recents: &[RecentQuickConnection],
    connection: &QuickConnection,
    used_at: i64,
) -> Vec<RecentQuickConnection> {
    let key = recent_key(connection);
    let mut next = vec![RecentQuickConnection {
        connection: connection.clone(),
        last_used_at: used_at,
    }];
    next.extend(
        recents
            .iter()
            .filter(|recent| recent_key(&recent.connection) != key)
            .cloned(),
    );
    next.truncate(QUICK_CONNECT_RECENTS_LIMIT);
    next
}

/// Forgets a quick connection.
pub fn remove_recent_quick_connection(
    recents: &[RecentQuickConnection],
    connection: &QuickConnection,
) -> Vec<RecentQuickConnection> {
    let key = recent_key(connection);
    recents
        .iter()
        .filter(|recent| recent_key(&recent.connection) != key)
        .cloned()
        .collect()
}

/// Reads the stored list, keeping only the well-formed entries.
pub fn parse_recent_quick_connections(raw: Option<&str>) -> Vec<RecentQuickConnection> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| {
            let Value::Object(fields) = &item else {
                return None;
            };
            if !fields.get("title")?.is_string()
                || !fields.get("username")?.is_string()
                || !fields.get("host")?.is_string()
                || !fields.get("port")?.is_number()
                || !fields.get("lastUsedAt")?.is_number()
            {
                return None;
            }
            serde_json::from_value(item).ok()
        })
        .take(QUICK_CONNECT_RECENTS_LIMIT)
        .collect()
}

/// Serializes the recent list for the WebView's storage adapter.
pub fn serialize_recent_quick_connections(
    recents: &[RecentQuickConnection],
) -> Result<String, serde_json::Error> {
    serde_json::to_string(recents)
}

/// Records what got a quick connection in. A reusable method replaces `auto`
/// on the tab, so later SFTP and reconnect work can reuse it.
pub fn apply_quick_connect_auth(tab: &Tab, auth: &SshAuthUsed) -> Tab {
    if tab
        .connection
        .as_ref()
        .and_then(|connection| connection.auth_method.as_deref())
        != Some("auto")
    {
        return tab.clone();
    }
    let Some(connection) = tab
        .saved_field("connection")
        .and_then(Value::as_object)
        .cloned()
    else {
        return tab.clone();
    };
    let mut next = tab.clone();
    next.set_saved_field("quickConnectAuth", serde_json::to_value(auth).ok());
    if auth.reusable {
        let mut connection = connection;
        insert_serialized(&mut connection, "authMethod", &auth.method);
        if auth.method == "key" {
            if let Some(path) = &auth.private_key_path {
                insert_serialized(&mut connection, "privateKeyPath", path);
            } else {
                connection.shift_remove("privateKeyPath");
            }
        } else {
            connection.shift_remove("privateKeyPath");
        }
        next.set_saved_field("connection", Some(Value::Object(connection)));
    }
    next
}

fn empty_saved_profile() -> SavedProfile {
    SavedProfile {
        id: String::new(),
        name: String::new(),
        group_id: None,
        group_path: None,
        legacy_group: String::new(),
        connection_type: "ssh".to_owned(),
        host: None,
        port: None,
        username: None,
        password: None,
        ignore_saved_password: false,
        remember_password: false,
        auth_method: None,
        agent_forward: false,
        private_key_path: None,
        private_key_passphrase: None,
        certificate_path: None,
        keepalive_interval_secs: 15,
        keepalive_count_max: 3,
        server_monitor_visible: false,
        use_jump_host: None,
        legacy_jump_host: None,
        jump_hosts: Vec::new(),
        sudo_autofill: true,
        sudo_password: None,
        clear_sudo_password: false,
        encoding: None,
        terminal_theme: None,
        login_script: None,
        ssh_shell_integration: None,
        legacy_shell_integration: false,
        session_log: None,
        sftp_start_directory: None,
        highlight_set: None,
        proxy: Default::default(),
        tab_icon: None,
        distro: None,
        serial: None,
        local_terminal: None,
    }
}

fn draft_auth_method(private_key_path: Option<&str>) -> &'static str {
    if private_key_path.is_some() {
        "key"
    } else {
        "password"
    }
}

/// A profile to save for a quick connection tab, with the method that worked.
pub fn draft_profile_from_tab(tab: &Tab) -> SavedProfile {
    let auth: Option<SshAuthUsed> = tab
        .saved_field("quickConnectAuth")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok());
    let connection_auth = connection_field::<String>(tab, "authMethod");
    let auth_method = connection_auth
        .filter(|method| method != "auto")
        .or_else(|| auth.as_ref().map(|auth| auth.method.clone()))
        .unwrap_or_else(|| {
            let key = connection_field::<String>(tab, "privateKeyPath");
            draft_auth_method(key.as_deref()).to_owned()
        });
    let private_key_path = auth
        .as_ref()
        .and_then(|auth| auth.private_key_path.clone())
        .or_else(|| connection_field(tab, "privateKeyPath"));
    let jump_hosts: Vec<JumpHostConnection> =
        connection_field(tab, "jumpHosts").unwrap_or_default();
    let saved_jump_hosts: Vec<SavedJumpHost> = jump_hosts
        .iter()
        .map(|jump| SavedJumpHost {
            host: jump.host.clone(),
            port: jump.port,
            username: jump.username.clone(),
            auth_method: if jump.auth_method == "auto" {
                draft_auth_method(jump.private_key_path.as_deref()).to_owned()
            } else {
                jump.auth_method.clone()
            },
            private_key_path: jump.private_key_path.clone(),
            private_key_passphrase: None,
            certificate_path: None,
            password: None,
            profile_id: None,
        })
        .collect();
    let mut profile = empty_saved_profile();
    profile.name = tab.title.clone();
    profile.host = connection_field(tab, "host");
    profile.port = Some(connection_field(tab, "port").unwrap_or(DEFAULT_SSH_PORT));
    profile.username = connection_field(tab, "username");
    profile.auth_method = Some(auth_method.clone());
    profile.agent_forward = connection_field(tab, "agentForward").unwrap_or(false);
    profile.private_key_path = (auth_method == "key").then_some(private_key_path).flatten();
    profile.keepalive_interval_secs = connection_field(tab, "keepaliveIntervalSecs").unwrap_or(15);
    profile.keepalive_count_max = connection_field(tab, "keepaliveCountMax").unwrap_or(3);
    profile.use_jump_host = Some(!saved_jump_hosts.is_empty());
    profile.jump_hosts = saved_jump_hosts;
    profile
}

/// A profile prefilled with what was typed, for the full connection dialog.
pub fn draft_profile_from_input(
    input: &str,
    config_hosts: &[SshConfigHost],
) -> Option<SavedProfile> {
    let target = parse_quick_connect_input(input)?;
    let connection =
        resolve_quick_connect_target(&target, config_hosts).unwrap_or_else(|| QuickConnection {
            title: format_endpoint(&QuickConnectEndpoint {
                username: target.username.clone(),
                host: target.host.clone(),
                port: target.port,
            }),
            username: String::new(),
            host: find_ssh_config_host(config_hosts, &target.host)
                .and_then(|host| host.host.clone())
                .unwrap_or_else(|| target.host.clone()),
            port: target.port.unwrap_or(DEFAULT_SSH_PORT),
            private_key_path: None,
            agent_forward: None,
            keepalive_interval_secs: None,
            keepalive_count_max: None,
            jump_hosts: None,
        });
    Some(draft_profile_from_tab(&build_quick_connect_tab(
        &connection,
    )))
}

#[cfg(test)]
mod tests;
