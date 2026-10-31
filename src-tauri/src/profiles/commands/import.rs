//! Importing profiles from an OpenSSH config file.

use crate::profiles::groups::resolve_path;
use crate::profiles::parser::build_ssh_config_import_preview;
use crate::profiles::storage::{load_profiles, sanitize_profile, upsert_profile};
use crate::profiles::types::{SavedProfile, SshConfigImportOptions, SshConfigImportResult};
use proxy::ImportedProxies;
use std::collections::HashSet;

mod proxy;

pub(super) fn import_ssh_config_profiles_blocking(
    options: SshConfigImportOptions,
) -> Result<SshConfigImportResult, String> {
    let preview = build_ssh_config_import_preview(options.source_path)?;
    let selected_hosts = options
        .selected_hosts
        .into_iter()
        .collect::<HashSet<String>>();
    let import_all = selected_hosts.is_empty();
    let group_path = options
        .group
        .as_deref()
        .unwrap_or("Imported from SSH config");
    let group_id = crate::db::write(|transaction| resolve_path(transaction, group_path))?;
    let mut profiles = load_profiles()?;
    let mut proxies = ImportedProxies::load()?;
    let mut imported = 0;
    let mut updated = 0;
    let mut skipped = 0;
    // (profile id, profile name, forwards) for hosts whose forwards to import.
    let mut forward_sources: Vec<(String, String, Vec<crate::tunnel::forwards::ForwardSpec>)> =
        Vec::new();
    let mut changed_profiles = Vec::new();

    for host in preview.hosts {
        if host.skipped || (!import_all && !selected_hosts.contains(&host.host_pattern)) {
            skipped += 1;
            continue;
        }

        let Some(hostname) = host.host else {
            skipped += 1;
            continue;
        };

        let existing_index = profiles
            .iter()
            .position(|profile| profile.name == host.name && profile.group_id == group_id);

        if let Some(index) = existing_index {
            if !options.overwrite_existing {
                // The profile stays as it is, but its forwards can still be added.
                forward_sources.push((
                    profiles[index].id.clone(),
                    profiles[index].name.clone(),
                    host.forwards,
                ));
                skipped += 1;
                continue;
            }
        }

        // An overwritten profile keeps its host's distribution while it still
        // points at the same host.
        let distro = existing_index
            .and_then(|index| profiles[index].distro_at(Some(&hostname), Some(host.port)));
        let profile = SavedProfile {
            id: existing_index
                .map(|index| profiles[index].id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name: host.name,
            group_id: group_id.clone(),
            group_path: None,
            legacy_group: String::new(),
            connection_type: "ssh".to_string(),
            host: Some(hostname),
            port: Some(host.port),
            username: host.username,
            password: None,
            ignore_saved_password: false,
            remember_password: false,
            auth_method: Some(host.auth_method),
            agent_forward: host.agent_forward,
            private_key_path: host.private_key_path,
            private_key_passphrase: None,
            certificate_path: host.certificate_path,
            keepalive_interval_secs: host.keepalive_interval_secs,
            keepalive_count_max: host.keepalive_count_max,
            server_monitor_visible: false,
            use_jump_host: Some(!host.jump_hosts.is_empty()),
            legacy_jump_host: None,
            jump_hosts: host.jump_hosts,
            sudo_autofill: existing_index.is_none_or(|index| profiles[index].sudo_autofill),
            sudo_password: None,
            clear_sudo_password: false,
            encoding: existing_index.and_then(|index| profiles[index].encoding.clone()),
            terminal_theme: existing_index.and_then(|index| profiles[index].terminal_theme.clone()),
            login_script: existing_index.and_then(|index| profiles[index].login_script.clone()),
            ssh_shell_integration: existing_index
                .and_then(|index| profiles[index].ssh_shell_integration),
            legacy_shell_integration: existing_index
                .is_some_and(|index| profiles[index].legacy_shell_integration),
            session_log: existing_index.and_then(|index| profiles[index].session_log.clone()),
            sftp_start_directory: existing_index
                .and_then(|index| profiles[index].sftp_start_directory.clone()),
            highlight_set: existing_index.and_then(|index| profiles[index].highlight_set.clone()),
            // A host whose config sets no proxy keeps the one chosen in tTerm.
            proxy: match &host.proxy {
                Some(proxy) => proxies.choice(proxy),
                None => existing_index
                    .map(|index| profiles[index].proxy.clone())
                    .unwrap_or_default(),
            },
            tab_icon: existing_index.and_then(|index| profiles[index].tab_icon.clone()),
            distro,
            serial: None,
        };

        forward_sources.push((profile.id.clone(), profile.name.clone(), host.forwards));

        if let Some(index) = existing_index {
            profiles[index] = profile.clone();
            updated += 1;
        } else {
            profiles.push(profile.clone());
            imported += 1;
        }
        changed_profiles.push(profile);
    }

    crate::db::write(|transaction| {
        proxies.save(transaction)?;
        for profile in &changed_profiles {
            upsert_profile(transaction, profile)?;
        }
        Ok(())
    })?;

    let (tunnels_imported, tunnels_skipped) = if options.import_forwards {
        let rules = forward_sources
            .iter()
            .flat_map(|(profile_id, profile_name, specs)| {
                specs.iter().map(|spec| {
                    crate::tunnel::forwards::rule_from_spec(profile_id, profile_name, spec)
                })
            })
            .collect();
        crate::tunnel::forwards::add_rules(rules)?
    } else {
        (0, 0)
    };

    let mut sanitized_profiles = profiles.clone();
    for profile in &mut sanitized_profiles {
        sanitize_profile(profile);
    }

    Ok(SshConfigImportResult {
        imported,
        updated,
        skipped,
        tunnels_imported,
        tunnels_skipped,
        profiles: sanitized_profiles,
    })
}
