//! Builds the import preview entry of one `Host` block.

use super::jump::{parse_proxy_jump, JumpTarget};
use super::parse::SshConfig;
use super::paths::expand_home_path;
use super::pattern::is_wildcard_host;
use super::proxy_command::{classify, ProxyCommandUse};
use crate::profiles::types::{
    default_keepalive_count, default_keepalive_interval, RawSshHost, SshConfigImportHost,
    SshConfigImportProxy,
};
use crate::tunnel::forwards::ForwardSpec;
use std::collections::HashMap;

/// The first value of an option, trimmed, unless it is empty.
pub(super) fn first_option(options: &HashMap<String, Vec<String>>, key: &str) -> Option<String> {
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

/// The `ProxyJump` to import and the proxy, from the host's `ProxyJump` or
/// `ProxyCommand`, of which it has one at most. A `ProxyCommand` of `ssh -W`
/// becomes the `ProxyJump`.
fn proxy_settings(
    raw: &RawSshHost,
    warnings: &mut Vec<String>,
) -> (Option<String>, Option<SshConfigImportProxy>) {
    if let Some(proxy_jump) = first_option(&raw.options, "proxyjump") {
        return (Some(proxy_jump), None);
    }
    let Some(command) = first_option(&raw.options, "proxycommand") else {
        return (None, None);
    };
    match classify(&command) {
        ProxyCommandUse::Direct => (None, Some(SshConfigImportProxy::Direct)),
        ProxyCommandUse::Server { kind, host, port } => (
            None,
            Some(SshConfigImportProxy::Server { kind, host, port }),
        ),
        ProxyCommandUse::Jump(jump) => (Some(jump), None),
        ProxyCommandUse::Command(command) => {
            let sample = crate::proxy::command::CommandTarget {
                host: "host",
                port: 22,
                username: "user",
            };
            match crate::proxy::command::expand(&command, &sample) {
                Ok(_) => (None, Some(SshConfigImportProxy::Command { command })),
                Err(reason) => {
                    warnings.push(format!("ProxyCommand was not imported: {reason}"));
                    (None, None)
                }
            }
        }
    }
}

pub(super) fn build_import_host(
    raw: RawSshHost,
    config: &SshConfig,
    existing_profiles: &[crate::profiles::types::SavedProfile],
) -> SshConfigImportHost {
    let mut warnings = raw.warnings.clone();
    let name = raw.pattern.clone();
    let hostname = first_option(&raw.options, "hostname");
    let host = hostname
        .clone()
        .or_else(|| (!is_wildcard_host(&raw.pattern)).then(|| raw.pattern.clone()));
    let username = first_option(&raw.options, "user");
    let port = parse_u16_option(first_option(&raw.options, "port")).unwrap_or(22);
    let private_key_path = first_option(&raw.options, "identityfile")
        .map(|value| expand_home_path(&value).to_string_lossy().to_string());
    let auth_method = if private_key_path.is_some() {
        "key".to_string()
    } else {
        "password".to_string()
    };
    let certificate_path = first_option(&raw.options, "certificatefile")
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
    let agent_forward =
        parse_forward_agent(first_option(&raw.options, "forwardagent"), &mut warnings);
    let keepalive_interval_secs =
        parse_u32_option(first_option(&raw.options, "serveraliveinterval"))
            .unwrap_or_else(default_keepalive_interval);
    let keepalive_count_max = parse_u32_option(first_option(&raw.options, "serveralivecountmax"))
        .unwrap_or_else(default_keepalive_count);
    let mut skipped = false;
    let mut skip_reason = None;

    if is_wildcard_host(&raw.pattern) {
        skipped = true;
        skip_reason =
            Some("Wildcard Host patterns are not imported as saved profiles; their options apply to the hosts they match.".to_string());
    } else if host.is_none() {
        skipped = true;
        skip_reason =
            Some("HostName is missing and the Host alias cannot be used as a host.".to_string());
    }

    if username.is_none() {
        warnings.push("User is missing; username must be filled before connecting.".to_string());
    }

    let (proxy_jump, proxy) = proxy_settings(&raw, &mut warnings);
    let target = JumpTarget {
        alias: &raw.pattern,
        private_key_path: private_key_path.as_deref(),
        certificate_path: certificate_path.as_deref(),
    };
    let jump_hosts = parse_proxy_jump(proxy_jump, config, &target, &mut warnings);
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
        proxy,
        warnings,
        unsupported_options: raw.unsupported_options,
        forwards,
        skipped,
        skip_reason,
        existing_profile_id,
    }
}
