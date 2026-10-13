//! Importing hosts from an OpenSSH config file: `paths` finds the file,
//! `include` reads it with its includes, `words` and `parse` split it into
//! `Host` blocks, and `host` turns each block into a preview entry, with
//! `proxy_command` reading what its `ProxyCommand` does.

mod host;
mod include;
mod parse;
pub(crate) mod paths;
mod proxy_command;
mod words;

use super::storage::load_profiles;
use super::types::SshConfigImportPreview;
use host::build_import_host;
use include::read_ssh_config_with_includes;
use parse::parse_ssh_config;
use paths::normalize_import_source;
use std::collections::HashSet;

pub(crate) fn build_ssh_config_import_preview(
    source_path: Option<String>,
) -> Result<SshConfigImportPreview, String> {
    let source = normalize_import_source(source_path)?;
    let (content, include_warnings) =
        read_ssh_config_with_includes(&source, &mut HashSet::new(), 0)?;
    let (raw_hosts, defaults, mut warnings) = parse_ssh_config(&content);
    warnings.extend(include_warnings);
    let existing_profiles = load_profiles()?;
    let hosts = raw_hosts
        .into_iter()
        .map(|raw| build_import_host(raw, &defaults, &existing_profiles))
        .collect();

    Ok(SshConfigImportPreview {
        source_path: source.to_string_lossy().to_string(),
        hosts,
        warnings,
    })
}

#[cfg(test)]
mod tests;
