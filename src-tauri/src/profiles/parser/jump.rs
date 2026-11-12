//! Reads a host's `ProxyJump` into the jump hosts tTerm saves.
//!
//! `ssh` reads the config for each jump host by the name `ProxyJump` gives
//! it, so a name like `bastion` takes its `HostName`, `Port`, `User` and
//! `IdentityFile` from the blocks that match it. The first jump host's own
//! `ProxyJump` comes before it in the chain; `ssh` passes the rest of the
//! list as `-J`, which overrides their own.

use super::host::first_option;
use super::parse::SshConfig;
use super::resolve::resolve_host;
use super::tokens::{key_file_path, PathTokens};
use crate::profiles::types::SavedJumpHost;
use std::collections::HashMap;

/// How deep the jump hosts of jump hosts are followed.
const MAX_CHAIN_DEPTH: usize = 8;

/// The host whose jump hosts are read, and the key it signs in with, which a
/// jump host without a key file of its own uses too.
pub(super) struct JumpTarget<'a> {
    pub alias: &'a str,
    pub private_key_path: Option<&'a str>,
    pub certificate_path: Option<&'a str>,
}

/// One `[user@]host[:port]` entry of a `ProxyJump` list.
struct JumpSpec {
    user: Option<String>,
    host: String,
    port: Option<u16>,
}

/// The host of the last jump host in a `ProxyJump` value as it is written,
/// which `ssh` fills in for `%j`, or empty without one.
pub(super) fn last_jump_host(proxy_jump: Option<&str>) -> String {
    proxy_jump
        .filter(|value| !value.trim().eq_ignore_ascii_case("none"))
        .and_then(|value| value.split(',').map(str::trim).rfind(|hop| !hop.is_empty()))
        .map(|hop| parse_jump_spec(hop).host)
        .unwrap_or_default()
}

fn parse_jump_spec(raw_jump: &str) -> JumpSpec {
    let (user, host_part) = raw_jump
        .rsplit_once('@')
        .map(|(user, host)| (Some(user.to_string()), host.to_string()))
        .unwrap_or((None, raw_jump.to_string()));

    let (host, port) = if host_part.starts_with('[') {
        match host_part.find(']') {
            Some(end) => (
                host_part[1..end].to_string(),
                host_part[end + 1..]
                    .strip_prefix(':')
                    .and_then(|value| value.parse::<u16>().ok()),
            ),
            None => (host_part, None),
        }
    } else if let Some((host, port)) = host_part.rsplit_once(':') {
        match port.parse::<u16>().ok() {
            Some(port) => (host.to_string(), Some(port)),
            None => (host_part, None),
        }
    } else {
        (host_part, None)
    };
    JumpSpec { user, host, port }
}

/// Whether some jump host signs in with the target's key or with a password,
/// or as the local user, which the import warns about once each.
#[derive(Default)]
struct BorrowedAuth {
    target_key: bool,
    password: bool,
    local_user: bool,
}

struct Chain<'a> {
    config: &'a SshConfig,
    target: &'a JumpTarget<'a>,
    /// The names on the way to the current entry, to stop at a loop.
    visited: Vec<String>,
    borrowed: BorrowedAuth,
    warnings: &'a mut Vec<String>,
}

impl Chain<'_> {
    /// The jump hosts of one `ProxyJump` value, in the order they connect.
    fn read(&mut self, proxy_jump: &str, depth: usize) -> Vec<SavedJumpHost> {
        let mut hops = Vec::new();
        if proxy_jump.trim().eq_ignore_ascii_case("none") {
            return hops;
        }
        for (index, raw_jump) in proxy_jump.split(',').map(str::trim).enumerate() {
            if raw_jump.is_empty() {
                continue;
            }
            let spec = parse_jump_spec(raw_jump);
            if spec.host.is_empty() {
                self.warnings.push(format!(
                    "ProxyJump entry '{raw_jump}' has no host and was skipped."
                ));
                continue;
            }
            if self
                .visited
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&spec.host))
            {
                self.warnings.push(format!(
                    "ProxyJump entry '{raw_jump}' leads back to a host already on the way and was skipped."
                ));
                continue;
            }

            let resolved = resolve_host(self.config, &spec.host);
            if index == 0
                && let Some(own_jump) = first_option(&resolved.options, "proxyjump") {
                    if depth >= MAX_CHAIN_DEPTH {
                        self.warnings.push(format!(
                            "The ProxyJump of '{}' was not followed: jump hosts go deeper than {MAX_CHAIN_DEPTH}.",
                            spec.host
                        ));
                    } else {
                        self.visited.push(spec.host.clone());
                        hops.extend(self.read(&own_jump, depth + 1));
                        self.visited.pop();
                    }
                }
            hops.push(self.hop(spec, &resolved.options));
        }
        hops
    }

    fn hop(&mut self, spec: JumpSpec, options: &HashMap<String, Vec<String>>) -> SavedJumpHost {
        let host = first_option(options, "hostname").unwrap_or_else(|| spec.host.clone());
        let port = spec
            .port
            .or_else(|| first_option(options, "port").and_then(|port| port.parse().ok()))
            .unwrap_or(22);
        let user = spec.user.or_else(|| first_option(options, "user"));
        let jump_host = last_jump_host(first_option(options, "proxyjump").as_deref());
        let tokens = PathTokens {
            alias: &spec.host,
            hostname: &host,
            port,
            user: user.as_deref().unwrap_or(&self.config.local_user),
            local_user: &self.config.local_user,
            local_host: &self.config.local_host,
            jump_host: &jump_host,
        };
        let (private_key_path, certificate_path) =
            match key_file_path(options, "identityfile", &tokens, self.warnings) {
                Some(key) => (
                    Some(key),
                    key_file_path(options, "certificatefile", &tokens, self.warnings),
                ),
                None => {
                    if self.target.private_key_path.is_some() {
                        self.borrowed.target_key = true;
                    } else {
                        self.borrowed.password = true;
                    }
                    (
                        self.target.private_key_path.map(str::to_string),
                        self.target.certificate_path.map(str::to_string),
                    )
                }
            };

        let username = user.unwrap_or_else(|| {
            self.borrowed.local_user = true;
            default_jump_user(self.config).to_string()
        });

        SavedJumpHost {
            host,
            port,
            username,
            auth_method: if private_key_path.is_some() {
                "key"
            } else {
                "password"
            }
            .to_string(),
            private_key_path,
            private_key_passphrase: None,
            certificate_path,
            password: None,
            profile_id: None,
        }
    }
}

/// The user for a jump host with no `User`: the local user, as `ssh` signs in
/// with, or `root` when the local user name cannot be found.
fn default_jump_user(config: &SshConfig) -> &str {
    if config.local_user.is_empty() {
        "root"
    } else {
        &config.local_user
    }
}

/// The jump hosts of `target`, from its `ProxyJump`.
pub(super) fn parse_proxy_jump(
    proxy_jump: Option<String>,
    config: &SshConfig,
    target: &JumpTarget,
    warnings: &mut Vec<String>,
) -> Vec<SavedJumpHost> {
    let Some(proxy_jump) = proxy_jump else {
        return Vec::new();
    };
    let mut chain = Chain {
        config,
        target,
        visited: vec![target.alias.to_string()],
        borrowed: BorrowedAuth::default(),
        warnings,
    };
    let hops = chain.read(&proxy_jump, 0);
    let borrowed = chain.borrowed;

    if borrowed.target_key {
        warnings.push(
            "ProxyJump entries without an IdentityFile of their own were imported with the target host private key; verify each jump host authentication setting.".to_string(),
        );
    }
    if borrowed.local_user {
        warnings.push(format!(
            "ProxyJump entries without a User were imported with the user name '{}', the local user ssh signs in as; verify each jump host username.",
            default_jump_user(config)
        ));
    }
    if borrowed.password {
        warnings.push(
            "ProxyJump entries without an IdentityFile were imported with password authentication; verify each jump host authentication setting.".to_string(),
        );
    }
    hops
}
