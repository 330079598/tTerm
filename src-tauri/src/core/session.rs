use crate::core::state::SessionKind;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct TerminalShellConfig {
    pub shell: String,
    pub custom_path: Option<String>,
    pub custom_args: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PtyConnectionOptions {
    #[serde(default, rename = "type")]
    pub connection_type: Option<String>,
    #[serde(default, alias = "profileName")]
    pub profile_name: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default, alias = "rememberPassword")]
    pub remember_password: Option<bool>,
    #[serde(default)]
    pub reconnect: Option<bool>,
    #[serde(default, alias = "reconnectDelaySecs")]
    pub reconnect_delay_secs: Option<u64>,
    #[serde(default, alias = "reconnectMaxDelaySecs")]
    pub reconnect_max_delay_secs: Option<u64>,
    #[serde(default, alias = "reconnectMaxRetries")]
    pub reconnect_max_retries: Option<u16>,
    #[serde(default, alias = "keepaliveIntervalSecs")]
    pub keepalive_interval_secs: Option<u16>,
    #[serde(default, alias = "keepaliveCountMax")]
    pub keepalive_count_max: Option<u16>,
    #[serde(default, alias = "privateKeyPath")]
    pub private_key_path: Option<String>,
    #[serde(default, alias = "privateKeyPassphrase")]
    pub private_key_passphrase: Option<String>,
    #[serde(default, alias = "terminalShell")]
    pub terminal_shell: Option<String>,
    #[serde(default, alias = "terminalShellCustomPath")]
    pub terminal_shell_custom_path: Option<String>,
    #[serde(default, alias = "terminalShellCustomArgs")]
    pub terminal_shell_custom_args: Option<String>,
}

impl Default for PtyConnectionOptions {
    fn default() -> Self {
        Self {
            connection_type: Some("terminal".to_string()),
            profile_name: None,
            host: None,
            port: None,
            username: None,
            password: None,
            remember_password: None,
            reconnect: None,
            reconnect_delay_secs: None,
            reconnect_max_delay_secs: None,
            reconnect_max_retries: None,
            keepalive_interval_secs: None,
            keepalive_count_max: None,
            private_key_path: None,
            private_key_passphrase: None,
            terminal_shell: None,
            terminal_shell_custom_path: None,
            terminal_shell_custom_args: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionPlan {
    pub kind: SessionKind,
    pub profile_name: String,
    pub host: Option<String>,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub remember_password: bool,
    pub reconnect: bool,
    pub reconnect_initial_delay: Duration,
    pub reconnect_max_delay: Duration,
    pub reconnect_max_retries: Option<u32>,
    pub keepalive_interval_secs: u16,
    pub keepalive_count_max: u16,
    pub private_key_path: Option<String>,
    pub private_key_passphrase: Option<String>,
    pub terminal_shell: Option<TerminalShellConfig>,
}

pub fn normalize_connection(
    connection: Option<PtyConnectionOptions>,
) -> Result<SessionPlan, String> {
    let connection = connection.unwrap_or_default();
    let kind = match connection.connection_type.as_deref() {
        Some("ssh") => SessionKind::Ssh,
        _ => SessionKind::Terminal,
    };

    let reconnect = connection
        .reconnect
        .unwrap_or(matches!(kind, SessionKind::Ssh));
    let reconnect_initial_delay_secs = connection.reconnect_delay_secs.unwrap_or(3).max(1);
    let reconnect_max_delay_secs = connection
        .reconnect_max_delay_secs
        .unwrap_or(60)
        .max(reconnect_initial_delay_secs);
    let reconnect_max_retries = match connection.reconnect_max_retries {
        Some(0) => None,
        Some(value) => Some(value as u32),
        None => Some(8),
    };
    let keepalive_interval_secs = connection.keepalive_interval_secs.unwrap_or(15).max(5);
    let keepalive_count_max = connection.keepalive_count_max.unwrap_or(3).max(1);

    let terminal_shell = Some(TerminalShellConfig {
        shell: connection
            .terminal_shell
            .unwrap_or_else(|| "auto".to_string())
            .trim()
            .to_string(),
        custom_path: connection
            .terminal_shell_custom_path
            .as_ref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        custom_args: connection
            .terminal_shell_custom_args
            .as_ref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
    });

    match kind {
        SessionKind::Terminal => Ok(SessionPlan {
            kind,
            profile_name: "terminal".to_string(),
            host: None,
            port: 0,
            username: None,
            password: None,
            remember_password: false,
            reconnect: false,
            reconnect_initial_delay: Duration::from_secs(reconnect_initial_delay_secs),
            reconnect_max_delay: Duration::from_secs(reconnect_max_delay_secs),
            reconnect_max_retries: None,
            keepalive_interval_secs,
            keepalive_count_max,
            private_key_path: None,
            private_key_passphrase: None,
            terminal_shell,
        }),
        SessionKind::Ssh => {
            let host = connection
                .host
                .as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .ok_or_else(|| "SSH host is required".to_string())?;
            let port = connection.port.unwrap_or(22);
            let username = connection
                .username
                .as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .ok_or_else(|| "SSH username is required".to_string())?;

            let profile_name = connection
                .profile_name
                .as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| format!("{}@{}:{}", username, host, port));

            let password = connection.password.filter(|v| !v.is_empty());
            let remember_password = connection.remember_password.unwrap_or(false);
            let private_key_path = connection.private_key_path.filter(|v| !v.is_empty());
            let private_key_passphrase = connection.private_key_passphrase.filter(|v| !v.is_empty());

            Ok(SessionPlan {
                kind,
                profile_name,
                host: Some(host),
                port,
                username: Some(username),
                password,
                remember_password,
                reconnect,
                reconnect_initial_delay: Duration::from_secs(reconnect_initial_delay_secs),
                reconnect_max_delay: Duration::from_secs(reconnect_max_delay_secs),
                reconnect_max_retries,
                keepalive_interval_secs,
                keepalive_count_max,
                private_key_path,
                private_key_passphrase,
                terminal_shell: None,
            })
        }
    }
}

pub fn resolve_ssh_password(
    app: &tauri::AppHandle,
    secret_state: &crate::ssh::SecretStoreState,
    plan: &mut SessionPlan,
) -> Result<(), String> {
    if !matches!(plan.kind, SessionKind::Ssh) {
        return Ok(());
    }

    if plan.private_key_path.is_some() {
        return Ok(());
    }

    let profile_id = plan.profile_name.clone();
    let password = if let Some(password) = plan.password.clone() {
        password
    } else {
        secret_state.get_password(app, &profile_id)?.ok_or_else(|| {
            format!(
                "No password provided and no saved password found for profile '{}'",
                plan.profile_name
            )
        })?
    };

    if plan.remember_password {
        let location = secret_state.save_password(app, &profile_id, &password)?;
        if matches!(location, crate::ssh::SecretLocation::Memory) {
            return Err(
                "Password persistence is unavailable. Enable the app vault or use a supported system credential store."
                    .to_string(),
            );
        }
    }

    plan.password = Some(password);
    Ok(())
}

pub fn next_backoff_delay(current: Duration, max: Duration) -> Duration {
    if current >= max {
        return max;
    }

    let doubled = current.saturating_mul(2);
    if doubled > max {
        max
    } else {
        doubled
    }
}
