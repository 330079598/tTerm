//! Reading `~/.ssh/config`: `paths` finds the files, `words` splits them into
//! words, `matching` checks `Match` criteria and `pattern` decides what is a
//! wildcard host.

mod host;
mod include;
mod jump;
mod known_hosts;
pub mod matching;
mod parse;
pub mod paths;
pub mod pattern;
mod proxy_command;
mod resolve;
mod tokens;
pub mod words;

use super::storage::load_profiles;
use super::types::{SshConfigImportHost, SshConfigImportPreview};
use crate::profiles::parser::paths::normalize_import_source;
use crate::profiles::parser::pattern::is_wildcard_host;
use host::build_import_host;
use include::read_ssh_config;
use known_hosts::named_known_hosts_files;
use parse::SshConfig;
use resolve::{import_entries, resolve_host};
use std::path::PathBuf;

pub fn build_ssh_config_import_preview(
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
pub fn resolve_config_host(name: &str) -> Result<Option<SshConfigImportHost>, String> {
    let name = name.trim();
    if name.is_empty() || is_wildcard_host(name) {
        return Ok(None);
    }
    let Some(source) =
        crate::profiles::parser::paths::home_dir().map(|home| home.join(".ssh").join("config"))
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
    let Some(source) =
        crate::profiles::parser::paths::home_dir().map(|home| home.join(".ssh").join("config"))
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
