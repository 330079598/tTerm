//! Building tabs from saved profiles and applying edits to open tabs, from
//! `src/lib/profileConnections.ts`.

use crate::config::sftp_start_directory::normalize_sftp_start_directory;
use crate::distro::is_distro_id;
use crate::profiles::types::{LocalTerminalOptions, SavedProfile};
use crate::serial::settings::normalize_serial_settings;
use crate::workspace::tab::{Tab, TabType};
use crate::workspace::tab_icons::{TabIconStyle, normalize_tab_icon_style};
use serde_json::{Map, Value};

/// The authentication method of a saved profile or jump host.
pub fn normalize_ssh_auth_method(value: Option<&str>) -> &'static str {
    match value {
        Some("key") => "key",
        Some("agent") => "agent",
        Some("interactive") => "interactive",
        _ => "password",
    }
}

/// A stored logging choice, or `None` to follow the logging setting.
pub fn normalize_session_log_policy(value: Option<&str>) -> Option<&'static str> {
    match value {
        Some("always") => Some("always"),
        Some("never") => Some("never"),
        _ => None,
    }
}

/// The icon a saved connection's tabs show, and the distribution its host runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileTabIcon {
    pub icon_style: Option<TabIconStyle>,
    pub distro_id: Option<String>,
}

fn insert<T: serde::Serialize + ?Sized>(connection: &mut Map<String, Value>, key: &str, value: &T) {
    if let Ok(value) = serde_json::to_value(value) {
        connection.insert(key.to_owned(), value);
    }
}

fn insert_optional<T: serde::Serialize>(
    connection: &mut Map<String, Value>,
    key: &str,
    value: Option<&T>,
) {
    if let Some(value) = value {
        insert(connection, key, value);
    }
}

fn profile_tab(tab_type: TabType, title: &str, connection: Map<String, Value>) -> Tab {
    let mut tab = Tab::new("", title, tab_type);
    tab.set_is_modified(Some(false));
    tab.set_saved_field("connection", Some(Value::Object(connection)));
    tab
}

fn base_connection(profile: &SavedProfile, connection_type: TabType) -> Map<String, Value> {
    let mut connection = Map::new();
    insert(&mut connection, "type", &connection_type);
    insert(&mut connection, "profileId", &profile.id);
    insert(&mut connection, "profileName", &profile.name);
    connection
}

fn serial_connection(profile: &SavedProfile) -> Map<String, Value> {
    let mut connection = base_connection(profile, TabType::Serial);
    insert_optional(
        &mut connection,
        "terminalTheme",
        profile.terminal_theme.as_ref(),
    );
    insert_optional(&mut connection, "encoding", profile.encoding.as_ref());
    if let Some(session_log) = normalize_session_log_policy(profile.session_log.as_deref()) {
        insert(&mut connection, "sessionLog", session_log);
    }
    insert_optional(
        &mut connection,
        "highlightSet",
        profile.highlight_set.as_ref(),
    );
    let raw = serde_json::to_value(&profile.serial).unwrap_or(Value::Null);
    insert(&mut connection, "serial", &normalize_serial_settings(&raw));
    connection
}

fn local_connection(profile: &SavedProfile) -> Map<String, Value> {
    let local = profile
        .local_terminal
        .clone()
        .unwrap_or(LocalTerminalOptions {
            shell: "auto".to_owned(),
            custom_path: None,
            custom_args: None,
            shell_id: None,
            wsl_distro: None,
            start_directory: None,
        })
        .normalized();
    let mut connection = base_connection(profile, TabType::Terminal);
    insert(&mut connection, "terminalShell", &local.shell);
    insert_optional(
        &mut connection,
        "terminalShellCustomPath",
        local.custom_path.as_ref(),
    );
    insert_optional(
        &mut connection,
        "terminalShellCustomArgs",
        local.custom_args.as_ref(),
    );
    insert_optional(&mut connection, "terminalShellId", local.shell_id.as_ref());
    insert_optional(
        &mut connection,
        "terminalShellWslDistro",
        local.wsl_distro.as_ref(),
    );
    insert_optional(&mut connection, "cwd", local.start_directory.as_ref());
    insert_optional(
        &mut connection,
        "terminalTheme",
        profile.terminal_theme.as_ref(),
    );
    insert_optional(
        &mut connection,
        "loginScript",
        profile.login_script.as_ref(),
    );
    if let Some(session_log) = normalize_session_log_policy(profile.session_log.as_deref()) {
        insert(&mut connection, "sessionLog", session_log);
    }
    insert_optional(
        &mut connection,
        "highlightSet",
        profile.highlight_set.as_ref(),
    );
    connection
}

fn ssh_connection(profile: &SavedProfile, connection_type: &TabType) -> Map<String, Value> {
    let mut connection = base_connection(profile, connection_type.clone());
    insert_optional(&mut connection, "host", profile.host.as_ref());
    insert_optional(&mut connection, "port", profile.port.as_ref());
    insert_optional(&mut connection, "username", profile.username.as_ref());
    insert(
        &mut connection,
        "authMethod",
        normalize_ssh_auth_method(profile.auth_method.as_deref()),
    );
    insert(&mut connection, "agentForward", &profile.agent_forward);
    if profile.auth_method.as_deref() == Some("key") {
        insert_optional(
            &mut connection,
            "privateKeyPath",
            profile.private_key_path.as_ref(),
        );
        insert_optional(
            &mut connection,
            "certificatePath",
            profile.certificate_path.as_ref(),
        );
    }
    insert(
        &mut connection,
        "keepaliveIntervalSecs",
        &profile.keepalive_interval_secs,
    );
    insert(
        &mut connection,
        "keepaliveCountMax",
        &profile.keepalive_count_max,
    );
    insert(
        &mut connection,
        "serverMonitorVisible",
        &profile.server_monitor_visible,
    );
    insert_optional(&mut connection, "encoding", profile.encoding.as_ref());
    insert_optional(
        &mut connection,
        "terminalTheme",
        profile.terminal_theme.as_ref(),
    );
    insert_optional(
        &mut connection,
        "loginScript",
        profile.login_script.as_ref(),
    );
    insert_optional(
        &mut connection,
        "sshShellIntegration",
        profile.ssh_shell_integration.as_ref(),
    );
    if let Some(session_log) = normalize_session_log_policy(profile.session_log.as_deref()) {
        insert(&mut connection, "sessionLog", session_log);
    }
    if let Some(directory) = normalize_sftp_start_directory(profile.sftp_start_directory.as_deref())
    {
        insert(&mut connection, "sftpStartDirectory", directory.as_str());
    }
    insert_optional(
        &mut connection,
        "highlightSet",
        profile.highlight_set.as_ref(),
    );
    if !profile.proxy.is_default() {
        insert(&mut connection, "proxy", &profile.proxy);
    }
    let use_jump_host = profile
        .use_jump_host
        .unwrap_or(!profile.jump_hosts.is_empty());
    if use_jump_host && !profile.jump_hosts.is_empty() {
        let jumps: Vec<Value> = profile
            .jump_hosts
            .iter()
            .map(|jump| {
                let mut saved = Map::new();
                insert(&mut saved, "host", &jump.host);
                insert(&mut saved, "port", &jump.port);
                insert(&mut saved, "username", &jump.username);
                insert(
                    &mut saved,
                    "authMethod",
                    normalize_ssh_auth_method(Some(&jump.auth_method)),
                );
                insert_optional(&mut saved, "privateKeyPath", jump.private_key_path.as_ref());
                insert_optional(
                    &mut saved,
                    "certificatePath",
                    jump.certificate_path.as_ref(),
                );
                insert_optional(&mut saved, "profileId", jump.profile_id.as_ref());
                Value::Object(saved)
            })
            .collect();
        connection.insert("jumpHosts".to_owned(), Value::Array(jumps));
    }
    connection
}

/// The icon a saved connection's tabs show, and the distribution its host runs.
pub fn profile_tab_icon(profile: &SavedProfile) -> ProfileTabIcon {
    if profile.connection_type != "ssh" {
        return ProfileTabIcon::default();
    }
    let normalized = profile
        .tab_icon
        .as_ref()
        .and_then(|style| serde_json::to_value(style).ok())
        .and_then(|value| normalize_tab_icon_style(&value));
    ProfileTabIcon {
        icon_style: normalized,
        distro_id: profile
            .distro
            .as_deref()
            .filter(|distro| is_distro_id(distro))
            .map(str::to_owned),
    }
}

/// A tab that opens a saved profile.
pub fn build_connection_from_profile(profile: &SavedProfile) -> Tab {
    let tab_type = TabType::from(profile.connection_type.clone());
    let connection = if tab_type == TabType::Serial {
        serial_connection(profile)
    } else if tab_type == TabType::Terminal {
        local_connection(profile)
    } else {
        ssh_connection(profile, &tab_type)
    };
    let icon = profile_tab_icon(profile);
    let mut tab = profile_tab(tab_type, &profile.name, connection);
    tab.set_icon_style(icon.icon_style);
    tab.set_distro_id(icon.distro_id);
    tab
}

fn connection_map(tab: &Tab) -> Option<Map<String, Value>> {
    tab.saved_field("connection")
        .and_then(Value::as_object)
        .cloned()
}

fn profile_id(tab: &Tab) -> Option<&str> {
    if tab.tab_type != TabType::Ssh {
        return None;
    }
    tab.connection.as_ref()?.profile_id.as_deref()
}

/// The open SSH tabs that share `tab`'s saved connection, `tab` among them.
pub fn same_connection_tabs(tabs: &[Tab], tab: &Tab) -> Vec<Tab> {
    let Some(shared_profile_id) = profile_id(tab) else {
        return vec![tab.clone()];
    };
    tabs.iter()
        .filter(|other| profile_id(other) == Some(shared_profile_id))
        .cloned()
        .collect()
}

fn value_string(connection: &Map<String, Value>, key: &str) -> Option<String> {
    connection
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn value_port(connection: &Map<String, Value>, key: &str) -> Option<u32> {
    connection
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn same_account(previous: &Map<String, Value>, next: &Map<String, Value>) -> bool {
    value_string(previous, "host") == value_string(next, "host")
        && value_port(previous, "port") == value_port(next, "port")
        && value_string(previous, "username") == value_string(next, "username")
        && value_string(previous, "authMethod") == value_string(next, "authMethod")
        && value_string(previous, "privateKeyPath") == value_string(next, "privateKeyPath")
}

const SSH_OPTIONAL_FIELDS: [&str; 10] = [
    "certificatePath",
    "privateKeyPath",
    "encoding",
    "terminalTheme",
    "loginScript",
    "sshShellIntegration",
    "sessionLog",
    "sftpStartDirectory",
    "highlightSet",
    "proxy",
];

fn merge_connection(
    previous: &Map<String, Value>,
    next: &Map<String, Value>,
    next_type: TabType,
) -> Map<String, Value> {
    let mut connection = previous.clone();
    for (key, value) in next {
        connection.insert(key.clone(), value.clone());
    }
    if next_type == TabType::Ssh {
        for key in SSH_OPTIONAL_FIELDS {
            if !next.contains_key(key) {
                connection.shift_remove(key);
            }
        }
    }
    connection
}

/// An open tab of an edited profile, now connecting with what was saved.
pub fn apply_profile_to_tab(tab: &Tab, profile: &SavedProfile) -> Tab {
    let Some(previous) = connection_map(tab) else {
        return tab.clone();
    };
    let built = build_connection_from_profile(profile);
    let Some(next) = connection_map(&built) else {
        return tab.clone();
    };
    let next_type = built.tab_type.clone();
    let mut connection = merge_connection(&previous, &next, next_type.clone());
    if !same_account(&previous, &next) {
        connection.shift_remove("password");
        connection.shift_remove("privateKeyPassphrase");
        connection.shift_remove("ignoreSavedPassword");
    }
    if next_type == TabType::Terminal
        && let Some(cwd) = value_string(&previous, "cwd")
    {
        connection.insert("cwd".to_owned(), Value::String(cwd));
    }

    let old_name = value_string(&previous, "profileName");
    let mut title = tab.title.clone();
    let mut duplicate_base_title = tab.duplicate_base_title.clone();
    if let Some(old_name) = old_name.filter(|old_name| old_name != &profile.name) {
        if title == old_name {
            title = profile.name.clone();
        } else if duplicate_base_title.as_deref() == Some(&old_name) && title.starts_with(&old_name)
        {
            title = format!("{}{}", profile.name, &title[old_name.len()..]);
            duplicate_base_title = Some(profile.name.clone());
        }
    }

    let mut updated = tab.clone();
    updated.set_title(title);
    updated.set_duplicate_base_title(duplicate_base_title);
    updated.set_saved_field("connection", Some(Value::Object(connection)));
    let icon = profile_tab_icon(profile);
    updated.set_icon_style(icon.icon_style);
    updated.set_distro_id(icon.distro_id);
    updated
}

#[cfg(test)]
mod tests;
