mod types;

pub use types::{
    JumpHostOptions, JumpHostPlan, PtyConnectionOptions, SessionPlan, TerminalShellConfig,
    jump_host_identity_secret_key, jump_host_secret_key, MAX_JUMP_HOSTS,
};

use crate::core::state::SessionKind;

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
            ignore_saved_password: false,
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
            let ignore_saved_password = connection.ignore_saved_password;
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
                ignore_saved_password,
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
    allow_legacy_fallback: bool,
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

    if allow_legacy_fallback {
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

            // Persist only when the user explicitly enabled "remember password".
            if plan.remember_password {
                let location = secret_state.save_password(app, secret_key, &password)?;
                if plan.remember_password && matches!(location, crate::ssh::SecretLocation::Memory)
                {
                    return Err(
                        "Password persistence is unavailable. Enable the app vault or use a supported system credential store."
                            .to_string(),
                    );
                }
            }
        } else if plan.ignore_saved_password {
            return Err(format!(
                "Password is required for profile '{}'",
                plan.profile_name
            ));
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

    // Resolve jump host passwords independently of target authentication.
    resolve_jump_host_passwords(app, secret_state, plan)?;

    Ok(())
}

/// Resolve jump host passwords from the secret store when not already provided.
fn resolve_jump_host_passwords(
    app: &tauri::AppHandle,
    secret_state: &crate::ssh::SecretStoreState,
    plan: &mut SessionPlan,
) -> Result<(), String> {
    let allow_legacy_fallback = plan.jump_hosts.len() == 1;

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
            if plan.remember_password {
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
            allow_legacy_fallback,
        )? {
            jump.password = Some(pw);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_session_tabs_migrates_legacy_jump_host_to_jump_hosts() {
        let options: PtyConnectionOptions = serde_json::from_value(serde_json::json!({
            "type": "ssh",
            "host": "target",
            "port": 22,
            "username": "user",
            "jumpHost": {
                "host": "bastion",
                "port": 22,
                "username": "jump"
            }
        }))
        .expect("options should deserialize");

        // jumpHost (singular) should be deserialized into jump_hosts via the From impl
        let plan = normalize_connection(Some(options)).expect("should normalize");
        assert_eq!(plan.jump_hosts.len(), 1);
        assert_eq!(plan.jump_hosts[0].host, "bastion");
    }
}
