//! The `known_hosts` files `~/.ssh/config` names for a host with
//! `UserKnownHostsFile` and `GlobalKnownHostsFile`, which tTerm reads when it
//! checks the host's key.

use super::host::first_option;
use super::jump::last_jump_host;
use super::parse::SshConfig;
use super::resolve::resolve_host;
use super::tokens::{PathTokens, expand_tokens};
use crate::profiles::parser::paths::expand_home_path;
use crate::profiles::parser::words::split_ssh_words;
use std::path::PathBuf;

const KNOWN_HOSTS_OPTIONS: [&str; 2] = ["userknownhostsfile", "globalknownhostsfile"];

/// The files the config names for a host known by any of `names` and
/// connected to on `port`, in the order they first appear. `none` and a path
/// with a token tTerm cannot fill in are left out.
pub(super) fn named_known_hosts_files(
    config: &SshConfig,
    names: &[&str],
    port: u16,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for name in names.iter().map(|name| name.trim()) {
        if name.is_empty() {
            continue;
        }
        let options = resolve_host(config, name).options;
        let hostname = first_option(&options, "hostname").unwrap_or_else(|| name.to_string());
        let user = first_option(&options, "user").unwrap_or_else(|| config.local_user.clone());
        let jump_host = last_jump_host(first_option(&options, "proxyjump").as_deref());
        let tokens = PathTokens {
            alias: name,
            hostname: &hostname,
            port,
            user: &user,
            local_user: &config.local_user,
            local_host: &config.local_host,
            jump_host: &jump_host,
        };
        for option in KNOWN_HOSTS_OPTIONS {
            let Some(value) = first_option(&options, option) else {
                continue;
            };
            for path in split_ssh_words(&value) {
                if path.eq_ignore_ascii_case("none") {
                    continue;
                }
                let Ok(path) = expand_tokens(&path, &tokens) else {
                    continue;
                };
                let path = expand_home_path(&path);
                if !files.contains(&path) {
                    files.push(path);
                }
            }
        }
    }
    files
}
