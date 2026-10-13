//! Builds the import preview entry of one `Host` block.

use super::paths::expand_home_path;
use crate::profiles::types::{
    default_keepalive_count, default_keepalive_interval, RawSshHost, SavedJumpHost,
    SshConfigDefaults, SshConfigImportHost,
};
use crate::tunnel::forwards::ForwardSpec;
use std::collections::HashMap;

fn first_option(options: &HashMap<String, Vec<String>>, key: &str) -> Option<String> {
    options
        .get(&key.to_ascii_lowercase())
        .and_then(|values| values.first())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_u16_option(value: Option<String>) -> Option<u16> {
    value.and_then(|value| value.parse::<u16>().ok())
}

fn parse_u32_option(value: Option<String>) -> Option<u32> {
    value.and_then(|value| value.parse::<u32>().ok())
}

/// OpenSSH also accepts a socket path or `$ENV_VAR` here; tTerm only forwards
/// the agent behind `SSH_AUTH_SOCK`, so those aren't enabled rather than
/// silently exposing a different agent's keys.
fn parse_forward_agent(value: Option<String>, warnings: &mut Vec<String>) -> bool {
    let Some(value) = value else {
        return false;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" => true,
        "no" | "false" => false,
        _ => {
            warnings.push(format!(
                "ForwardAgent '{value}' was not enabled: only the default agent (SSH_AUTH_SOCK) can be forwarded."
            ));
            false
        }
    }
}

fn is_wildcard_host(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?') || pattern.contains('!')
}

fn parse_proxy_jump(
    proxy_jump: Option<String>,
    defaults: &SshConfigDefaults,
    private_key_path: Option<&str>,
    certificate_path: Option<&str>,
    warnings: &mut Vec<String>,
) -> Vec<SavedJumpHost> {
    let Some(proxy_jump) = proxy_jump else {
        return Vec::new();
    };

    if proxy_jump.eq_ignore_ascii_case("none") {
        return Vec::new();
    }

    let jump_auth_method = if private_key_path.is_some() {
        warnings.push(
            "ProxyJump entries were imported with the target host private key; verify each jump host authentication setting.".to_string(),
        );
        "key"
    } else {
        warnings.push(
            "ProxyJump entries were imported with password authentication; verify each jump host authentication setting.".to_string(),
        );
        "password"
    };

    proxy_jump
        .split(',')
        .filter_map(|raw_jump| {
            let raw_jump = raw_jump.trim();
            if raw_jump.is_empty() {
                return None;
            }

            let (user_part, host_part) = raw_jump
                .rsplit_once('@')
                .map(|(user, host)| (Some(user.to_string()), host.to_string()))
                .unwrap_or((None, raw_jump.to_string()));

            let (host, port) = if host_part.starts_with('[') {
                if let Some(end) = host_part.find(']') {
                    let host = host_part[1..end].to_string();
                    let port = host_part[end + 1..]
                        .strip_prefix(':')
                        .and_then(|value| value.parse::<u16>().ok())
                        .unwrap_or(22);
                    (host, port)
                } else {
                    (host_part, 22)
                }
            } else if let Some((host, port)) = host_part.rsplit_once(':') {
                let parsed_port = port.parse::<u16>().ok();
                match parsed_port {
                    Some(port) => (host.to_string(), port),
                    None => (host_part, 22),
                }
            } else {
                (host_part, 22)
            };

            if host.is_empty() {
                warnings.push(format!(
                    "ProxyJump entry '{}' has no host and was skipped.",
                    raw_jump
                ));
                return None;
            }

            Some(SavedJumpHost {
                host,
                port,
                username: user_part
                    .or_else(|| defaults.user.clone())
                    .unwrap_or_else(|| "root".to_string()),
                auth_method: jump_auth_method.to_string(),
                private_key_path: private_key_path.map(str::to_string),
                private_key_passphrase: None,
                certificate_path: certificate_path.map(str::to_string),
                password: None,
            })
        })
        .collect()
}

/// Collects a host's forward directives; ones that cannot be understood become
/// warnings rather than failing the import.
fn parse_forwards(
    options: &HashMap<String, Vec<String>>,
    warnings: &mut Vec<String>,
) -> Vec<ForwardSpec> {
    use crate::tunnel::{forwards::parse_forward, types::TunnelKind};

    let mut forwards = Vec::new();
    for (key, label, kind) in [
        ("localforward", "LocalForward", TunnelKind::Local),
        ("remoteforward", "RemoteForward", TunnelKind::Remote),
        ("dynamicforward", "DynamicForward", TunnelKind::Dynamic),
    ] {
        for value in options.get(key).into_iter().flatten() {
            match parse_forward(kind, value) {
                Ok(spec) if !forwards.contains(&spec) => forwards.push(spec),
                Ok(_) => {}
                Err(reason) => warnings.push(format!("{label} '{value}' was skipped: {reason}.")),
            }
        }
    }
    forwards
}

pub(super) fn build_import_host(
    raw: RawSshHost,
    defaults: &SshConfigDefaults,
    existing_profiles: &[crate::profiles::types::SavedProfile],
) -> SshConfigImportHost {
    let mut warnings = raw.warnings.clone();
    let name = raw.pattern.clone();
    let hostname = first_option(&raw.options, "hostname");
    let host = hostname
        .clone()
        .or_else(|| (!is_wildcard_host(&raw.pattern)).then(|| raw.pattern.clone()));
    let username = first_option(&raw.options, "user").or_else(|| defaults.user.clone());
    let port = parse_u16_option(first_option(&raw.options, "port"))
        .or(defaults.port)
        .unwrap_or(22);
    let private_key_path = first_option(&raw.options, "identityfile")
        .or_else(|| defaults.identity_file.clone())
        .map(|value| expand_home_path(&value).to_string_lossy().to_string());
    let auth_method = if private_key_path.is_some() {
        "key".to_string()
    } else {
        "password".to_string()
    };
    let certificate_path = first_option(&raw.options, "certificatefile")
        .or_else(|| defaults.certificate_file.clone())
        .map(|value| expand_home_path(&value).to_string_lossy().to_string());
    // tTerm offers a certificate with the private key it certifies, so one
    // without an IdentityFile has nothing to go with.
    let certificate_path = match certificate_path {
        Some(path) if private_key_path.is_none() => {
            warnings.push(format!(
                "CertificateFile '{path}' was not imported because the host has no IdentityFile."
            ));
            None
        }
        certificate_path => certificate_path,
    };
    let agent_forward = parse_forward_agent(
        first_option(&raw.options, "forwardagent").or_else(|| defaults.forward_agent.clone()),
        &mut warnings,
    );
    let keepalive_interval_secs =
        parse_u32_option(first_option(&raw.options, "serveraliveinterval"))
            .or(defaults.server_alive_interval)
            .unwrap_or_else(default_keepalive_interval);
    let keepalive_count_max = parse_u32_option(first_option(&raw.options, "serveralivecountmax"))
        .or(defaults.server_alive_count_max)
        .unwrap_or_else(default_keepalive_count);
    let mut skipped = false;
    let mut skip_reason = None;

    if is_wildcard_host(&raw.pattern) {
        skipped = true;
        skip_reason =
            Some("Wildcard Host patterns are not imported as saved profiles.".to_string());
    } else if host.is_none() {
        skipped = true;
        skip_reason =
            Some("HostName is missing and the Host alias cannot be used as a host.".to_string());
    }

    if username.is_none() {
        warnings.push("User is missing; username must be filled before connecting.".to_string());
    }

    let jump_hosts = parse_proxy_jump(
        first_option(&raw.options, "proxyjump"),
        defaults,
        private_key_path.as_deref(),
        certificate_path.as_deref(),
        &mut warnings,
    );
    let forwards = parse_forwards(&raw.options, &mut warnings);
    let existing_profile_id = existing_profiles
        .iter()
        .find(|profile| profile.name == name)
        .map(|profile| profile.id.clone());

    SshConfigImportHost {
        host_pattern: raw.pattern,
        name,
        host,
        port,
        username,
        auth_method,
        private_key_path,
        certificate_path,
        agent_forward,
        keepalive_interval_secs,
        keepalive_count_max,
        jump_hosts,
        warnings,
        unsupported_options: raw.unsupported_options,
        forwards,
        skipped,
        skip_reason,
        existing_profile_id,
    }
}
