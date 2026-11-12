//! Importing hosts from an OpenSSH config file: `paths` finds the file,
//! `include` opens it and the files it includes, `words` and `parse` split
//! them into `Host` and `Match` blocks, `matching` checks `Match` criteria, `resolve`
//! gathers each host's options from the blocks that match it, and `host`
//! turns them into a preview entry, with `proxy_command` reading what its
//! `ProxyCommand` does, `jump` reading its `ProxyJump` and `tokens` filling in
//! its key file paths.

mod host;
mod include;
mod jump;
mod known_hosts;
mod parse;
mod proxy_command;
mod resolve;
mod tokens;

use super::storage::load_profiles;
use super::types::{SshConfigImportHost, SshConfigImportPreview};
use host::build_import_host;
use include::read_ssh_config;
use known_hosts::named_known_hosts_files;
use parse::SshConfig;
use resolve::{import_entries, resolve_host};
use std::path::PathBuf;
use tterm_core::profiles::parser::paths::normalize_import_source;
use tterm_core::profiles::parser::pattern::is_wildcard_host;

pub(crate) fn build_ssh_config_import_preview(
    source_path: Option<String>,
) -> Result<SshConfigImportPreview, String> {
    let source = normalize_import_source(source_path)?;
    let (config, warnings) = read_ssh_config(&source)?;
    let existing_profiles = load_profiles()?;
    let hosts = import_entries(&config)
        .into_iter()
        .map(|raw| build_import_host(raw, &config, &existing_profiles))
        .collect();

    Ok(SshConfigImportPreview {
        source_path: source.to_string_lossy().to_string(),
        hosts,
        warnings,
    })
}

/// What a config sets for a host by any name, listed in it or not, the way
/// `ssh` would read it for `ssh name`.
fn config_host_for(config: &SshConfig, name: &str) -> SshConfigImportHost {
    build_import_host(resolve_host(config, name), config, &[])
}

/// What `~/.ssh/config` sets for a host typed by any name, or `None` when
/// there is no config file or the name is a pattern rather than a host.
pub(crate) fn resolve_config_host(name: &str) -> Result<Option<SshConfigImportHost>, String> {
    let name = name.trim();
    if name.is_empty() || is_wildcard_host(name) {
        return Ok(None);
    }
    let Some(source) = tterm_core::profiles::parser::paths::home_dir()
        .map(|home| home.join(".ssh").join("config"))
    else {
        return Ok(None);
    };
    if !source.is_file() {
        return Ok(None);
    }
    let (config, _) = read_ssh_config(&source)?;
    Ok(Some(config_host_for(&config, name)))
}

/// The `known_hosts` files `~/.ssh/config` names with `UserKnownHostsFile`
/// and `GlobalKnownHostsFile` for a host known by any of `names` and
/// connected to on `port`. None when there is no config or it cannot be read,
/// since the default files are read either way.
pub(crate) fn config_known_hosts_files(names: &[&str], port: u16) -> Vec<PathBuf> {
    let Some(source) = tterm_core::profiles::parser::paths::home_dir()
        .map(|home| home.join(".ssh").join("config"))
    else {
        return Vec::new();
    };
    if !source.is_file() {
        return Vec::new();
    }
    match read_ssh_config(&source) {
        Ok((config, _)) => named_known_hosts_files(&config, names, port),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod include_tests;
#[cfg(test)]
mod jump_tests;
#[cfg(test)]
mod match_tests;
#[cfg(test)]
mod tests;
