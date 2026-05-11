use crate::core::state::SessionKind;
use serde::Deserialize;

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
pub struct TerminalShellConfig {
    pub shell: String,
    pub custom_path: Option<String>,
    pub custom_args: Option<String>,
}

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone)]
pub struct TerminalShellConfig;

/// Jump host (bastion) connection parameters deserialized from the frontend.
#[derive(Debug, Deserialize, Clone)]
#[serde(from = "RawJumpHostOptions")]
pub struct JumpHostOptions {
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default, alias = "authMethod")]
    pub auth_method: Option<String>,
    #[serde(default, alias = "privateKeyPath")]
    pub private_key_path: Option<String>,
    #[serde(default, alias = "privateKeyPassphrase")]
    pub private_key_passphrase: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct RawJumpHostOptions {
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default, alias = "authMethod")]
    auth_method: Option<String>,
    #[serde(default, alias = "privateKeyPath")]
    private_key_path: Option<String>,
    #[serde(default, alias = "privateKeyPassphrase")]
    private_key_passphrase: Option<String>,
}

impl From<RawJumpHostOptions> for JumpHostOptions {
    fn from(raw: RawJumpHostOptions) -> Self {
        Self {
            host: raw.host,
            port: raw.port,
            username: raw.username,
            password: raw.password,
            auth_method: raw.auth_method,
            private_key_path: raw.private_key_path,
            private_key_passphrase: raw.private_key_passphrase,
        }
    }
}

/// Resolved jump host plan used at connection time.
#[derive(Debug, Clone)]
pub struct JumpHostPlan {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub private_key_path: Option<String>,
    pub private_key_passphrase: Option<String>,
}

pub fn jump_host_secret_key(profile_id: Option<&str>, profile_name: &str) -> String {
    format!("{}:jump", profile_id.unwrap_or(profile_name))
}

pub fn jump_host_identity_secret_key(
    profile_id: Option<&str>,
    profile_name: &str,
    host: &str,
    port: u16,
    username: &str,
) -> String {
    let profile_key = profile_id.unwrap_or(profile_name);
    format!("{profile_key}:jump:{host}:{port}:{username}")
}

pub const MAX_JUMP_HOSTS: usize = 8;

#[derive(Debug, Deserialize, Clone)]
#[serde(from = "RawPtyConnectionOptions")]
pub struct PtyConnectionOptions {
    pub connection_type: Option<String>,
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub remember_password: Option<bool>,
    pub keepalive_interval_secs: Option<u16>,
    pub keepalive_count_max: Option<u16>,
    pub private_key_path: Option<String>,
    pub private_key_passphrase: Option<String>,
    /// Ordered jump host chain to tunnel through before reaching the target.
    pub jump_hosts: Vec<JumpHostOptions>,
    #[cfg(target_os = "windows")]
    pub terminal_shell: Option<String>,
    #[cfg(target_os = "windows")]
    pub terminal_shell_custom_path: Option<String>,
    #[cfg(target_os = "windows")]
    pub terminal_shell_custom_args: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct RawPtyConnectionOptions {
    #[serde(default, rename = "type")]
    connection_type: Option<String>,
    #[serde(default, alias = "profileId")]
    profile_id: Option<String>,
    #[serde(default, alias = "profileName")]
    profile_name: Option<String>,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default, alias = "rememberPassword")]
    remember_password: Option<bool>,
    #[serde(default, alias = "keepaliveIntervalSecs")]
    keepalive_interval_secs: Option<u16>,
    #[serde(default, alias = "keepaliveCountMax")]
    keepalive_count_max: Option<u16>,
    #[serde(default, alias = "privateKeyPath")]
    private_key_path: Option<String>,
    #[serde(default, alias = "privateKeyPassphrase")]
    private_key_passphrase: Option<String>,
    #[serde(default, alias = "jumpHost")]
    legacy_jump_host: Option<JumpHostOptions>,
    #[serde(default, alias = "jumpHosts")]
    jump_hosts: Vec<JumpHostOptions>,
    #[cfg(target_os = "windows")]
    #[serde(default, alias = "terminalShell")]
    terminal_shell: Option<String>,
    #[cfg(target_os = "windows")]
    #[serde(default, alias = "terminalShellCustomPath")]
    terminal_shell_custom_path: Option<String>,
    #[cfg(target_os = "windows")]
    #[serde(default, alias = "terminalShellCustomArgs")]
    terminal_shell_custom_args: Option<String>,
}

impl From<RawPtyConnectionOptions> for PtyConnectionOptions {
    fn from(raw: RawPtyConnectionOptions) -> Self {
        let jump_hosts = if raw.jump_hosts.is_empty() {
            raw.legacy_jump_host.into_iter().collect()
        } else {
            raw.jump_hosts
        };

        Self {
            connection_type: raw.connection_type,
            profile_id: raw.profile_id,
            profile_name: raw.profile_name,
            host: raw.host,
            port: raw.port,
            username: raw.username,
            password: raw.password,
            remember_password: raw.remember_password,
            keepalive_interval_secs: raw.keepalive_interval_secs,
            keepalive_count_max: raw.keepalive_count_max,
            private_key_path: raw.private_key_path,
            private_key_passphrase: raw.private_key_passphrase,
            jump_hosts,
            #[cfg(target_os = "windows")]
            terminal_shell: raw.terminal_shell,
            #[cfg(target_os = "windows")]
            terminal_shell_custom_path: raw.terminal_shell_custom_path,
            #[cfg(target_os = "windows")]
            terminal_shell_custom_args: raw.terminal_shell_custom_args,
        }
    }
}

impl Default for PtyConnectionOptions {
    fn default() -> Self {
        Self {
            connection_type: Some("terminal".to_string()),
            profile_id: None,
            profile_name: None,
            host: None,
            port: None,
            username: None,
            password: None,
            remember_password: None,
            keepalive_interval_secs: None,
            keepalive_count_max: None,
            private_key_path: None,
            private_key_passphrase: None,
            jump_hosts: Vec::new(),
            #[cfg(target_os = "windows")]
            terminal_shell: None,
            #[cfg(target_os = "windows")]
            terminal_shell_custom_path: None,
            #[cfg(target_os = "windows")]
            terminal_shell_custom_args: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionPlan {
    pub kind: SessionKind,
    pub profile_id: Option<String>,
    pub profile_name: String,
    pub host: Option<String>,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub remember_password: bool,
    pub keepalive_interval_secs: u16,
    pub keepalive_count_max: u16,
    pub private_key_path: Option<String>,
    pub private_key_passphrase: Option<String>,
    pub terminal_shell: Option<TerminalShellConfig>,
    /// Ordered resolved jump host chain; empty means direct connection.
    pub jump_hosts: Vec<JumpHostPlan>,
}

pub fn normalize_connection(
    connection: Option<PtyConnectionOptions>,
) -> Result<SessionPlan, String> {
    let connection = connection.unwrap_or_default();
    let kind = match connection.connection_type.as_deref() {
        Some("ssh") => SessionKind::Ssh,
        _ => SessionKind::Terminal,
    };

    let keepalive_interval_secs = connection.keepalive_interval_secs.unwrap_or(15).max(5);
    let keepalive_count_max = connection.keepalive_count_max.unwrap_or(3).max(1);

    #[cfg(target_os = "windows")]
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

    #[cfg(not(target_os = "windows"))]
    let terminal_shell = None;

    match kind {
        SessionKind::Terminal => Ok(SessionPlan {
            kind,
            profile_id: None,
            profile_name: "terminal".to_string(),
            host: None,
            port: 0,
            username: None,
            password: None,
            remember_password: false,
            keepalive_interval_secs,
            keepalive_count_max,
            private_key_path: None,
            private_key_passphrase: None,
            terminal_shell,
            jump_hosts: Vec::new(),
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
            let profile_id = connection
                .profile_id
                .as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());

            let password = connection.password.filter(|v| !v.is_empty());
            let remember_password = connection.remember_password.unwrap_or(false);
            let private_key_path = connection.private_key_path.filter(|v| !v.is_empty());
            let private_key_passphrase =
                connection.private_key_passphrase.filter(|v| !v.is_empty());

            let jump_hosts = normalize_jump_hosts(connection.jump_hosts)?;

            Ok(SessionPlan {
                kind,
                profile_id,
                profile_name,
                host: Some(host),
                port,
                username: Some(username),
                password,
                remember_password,
                keepalive_interval_secs,
                keepalive_count_max,
                private_key_path,
                private_key_passphrase,
                terminal_shell: None,
                jump_hosts,
            })
        }
    }
}

pub fn normalize_jump_hosts(jump_hosts: Vec<JumpHostOptions>) -> Result<Vec<JumpHostPlan>, String> {
    if jump_hosts.len() > MAX_JUMP_HOSTS {
        return Err(format!(
            "At most {MAX_JUMP_HOSTS} jump hosts are supported per connection"
        ));
    }

    jump_hosts.into_iter().map(normalize_jump_host).collect()
}

/// Validate and convert raw jump host options into a resolved plan.
/// Returns `Err` if required fields (host, username) are missing or empty.
fn normalize_jump_host(opts: JumpHostOptions) -> Result<JumpHostPlan, String> {
    let host = opts
        .host
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "Jump host is required".to_string())?
        .to_string();

    let port = opts.port.unwrap_or(22);

    let username = opts
        .username
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "Jump host username is required".to_string())?
        .to_string();

    let use_key = opts.auth_method.as_deref() == Some("key");
    let private_key_path = if use_key {
        opts.private_key_path.filter(|v| !v.is_empty())
    } else {
        None
    };
    let private_key_passphrase = if use_key {
        opts.private_key_passphrase.filter(|v| !v.is_empty())
    } else {
        None
    };
    let password = if use_key {
        None
    } else {
        opts.password.filter(|v| !v.is_empty())
    };

    Ok(JumpHostPlan {
        host,
        port,
        username,
        password,
        private_key_path,
        private_key_passphrase,
    })
}

pub fn load_saved_ssh_password(
    app: &tauri::AppHandle,
    secret_state: &crate::ssh::SecretStoreState,
    profile_id: Option<&str>,
    profile_name: Option<&str>,
) -> Result<Option<String>, String> {
    let profile_id = profile_id.map(str::trim).filter(|v| !v.is_empty());
    let profile_name = profile_name.map(str::trim).filter(|v| !v.is_empty());

    if let Some(profile_id) = profile_id {
        if let Some(password) = secret_state.get_password(app, profile_id)? {
            return Ok(Some(password));
        }
    }

    if let Some(profile_name) = profile_name {
        if let Some(password) = secret_state.get_password(app, profile_name)? {
            if let Some(profile_id) = profile_id {
                let _ = secret_state.save_password(app, profile_id, &password);
            }
            return Ok(Some(password));
        }
    }

    Ok(None)
}

pub fn load_saved_jump_host_password(
    app: &tauri::AppHandle,
    secret_state: &crate::ssh::SecretStoreState,
    profile_id: Option<&str>,
    profile_name: Option<&str>,
    host: Option<&str>,
    port: Option<u16>,
    username: Option<&str>,
) -> Result<Option<String>, String> {
    let profile_id = profile_id.map(str::trim).filter(|v| !v.is_empty());
    let profile_name = profile_name.map(str::trim).filter(|v| !v.is_empty());
    let host = host.map(str::trim).filter(|v| !v.is_empty());
    let username = username.map(str::trim).filter(|v| !v.is_empty());

    let canonical_identity_key = match (profile_id, profile_name, host, port, username) {
        (Some(profile_id), _, Some(host), Some(port), Some(username)) => Some(
            jump_host_identity_secret_key(Some(profile_id), "", host, port, username),
        ),
        (None, Some(profile_name), Some(host), Some(port), Some(username)) => Some(
            jump_host_identity_secret_key(None, profile_name, host, port, username),
        ),
        _ => None,
    };

    if let Some(secret_key) = canonical_identity_key.as_deref() {
        if let Some(password) = secret_state.get_password(app, secret_key)? {
            return Ok(Some(password));
        }
    }

    if let (Some(profile_name), Some(host), Some(port), Some(username)) =
        (profile_name, host, port, username)
    {
        let name_identity_key =
            jump_host_identity_secret_key(None, profile_name, host, port, username);
        if canonical_identity_key.as_deref() != Some(name_identity_key.as_str()) {
            if let Some(password) = secret_state.get_password(app, &name_identity_key)? {
                if let Some(secret_key) = canonical_identity_key.as_deref() {
                    let _ = secret_state.save_password(app, secret_key, &password);
                }
                return Ok(Some(password));
            }
        }
    }

    for legacy_key in [
        profile_id.map(|value| jump_host_secret_key(Some(value), "")),
        profile_name.map(|value| jump_host_secret_key(None, value)),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(password) = secret_state.get_password(app, &legacy_key)? {
            if let Some(secret_key) = canonical_identity_key.as_deref() {
                let _ = secret_state.save_password(app, secret_key, &password);
            }
            return Ok(Some(password));
        }
    }

    Ok(None)
}

pub fn resolve_ssh_password(
    app: &tauri::AppHandle,
    secret_state: &crate::ssh::SecretStoreState,
    plan: &mut SessionPlan,
) -> Result<(), String> {
    if !matches!(plan.kind, SessionKind::Ssh) {
        return Ok(());
    }

    if plan.private_key_path.is_none() {
        if let Some(password) = plan.password.clone() {
            let secret_key = plan
                .profile_id
                .as_deref()
                .unwrap_or(plan.profile_name.as_str());

            // Save password to persistent store when remember_password is enabled,
            // or automatically when a profile_id exists (so session restore can find it)
            if plan.remember_password || plan.profile_id.is_some() {
                let location = secret_state.save_password(app, secret_key, &password)?;
                if plan.remember_password && matches!(location, crate::ssh::SecretLocation::Memory)
                {
                    return Err(
                        "Password persistence is unavailable. Enable the app vault or use a supported system credential store."
                            .to_string(),
                    );
                }
            }
        } else {
            // Try to get password from secret store
            let password = load_saved_ssh_password(
                app,
                secret_state,
                plan.profile_id.as_deref(),
                Some(plan.profile_name.as_str()),
            )?
            .ok_or_else(|| {
                format!(
                    "No password provided and no saved password found for profile '{}'",
                    plan.profile_name
                )
            })?;

            plan.password = Some(password);
        }
    }

    // Resolve jump host credentials independently of target authentication.
    resolve_jump_host_passwords(app, secret_state, plan)?;

    Ok(())
}

/// Resolve jump host passwords from the secret store when not already provided.
fn resolve_jump_host_passwords(
    app: &tauri::AppHandle,
    secret_state: &crate::ssh::SecretStoreState,
    plan: &mut SessionPlan,
) -> Result<(), String> {
    for jump in plan.jump_hosts.iter_mut() {
        if jump.private_key_path.is_some() {
            continue;
        }

        let secret_key = jump_host_identity_secret_key(
            plan.profile_id.as_deref(),
            plan.profile_name.as_str(),
            &jump.host,
            jump.port,
            &jump.username,
        );

        if let Some(pw) = &jump.password {
            if plan.remember_password || plan.profile_id.is_some() {
                let location = secret_state.save_password(app, &secret_key, pw)?;
                if plan.remember_password && matches!(location, crate::ssh::SecretLocation::Memory)
                {
                    return Err(
                        "Jump host password persistence is unavailable. Enable the app vault or use a supported system credential store."
                            .to_string(),
                    );
                }
            }
            continue;
        }

        if let Some(pw) = load_saved_jump_host_password(
            app,
            secret_state,
            plan.profile_id.as_deref(),
            Some(plan.profile_name.as_str()),
            Some(jump.host.as_str()),
            Some(jump.port),
            Some(jump.username.as_str()),
        )? {
            jump.password = Some(pw);
        }
    }

    Ok(())
}
