use super::storage::load_profiles_from_disk;
use super::types::{
    default_keepalive_count, default_keepalive_interval, RawSshHost, SavedJumpHost,
    SshConfigDefaults, SshConfigImportHost, SshConfigImportPreview,
};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(crate) fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
        .or_else(|| {
            let drive = env::var_os("HOMEDRIVE")?;
            let path = env::var_os("HOMEPATH")?;
            Some(PathBuf::from(format!(
                "{}{}",
                drive.to_string_lossy(),
                path.to_string_lossy()
            )))
        })
}

pub(crate) fn expand_home_path(path: &str) -> PathBuf {
    let trimmed = path.trim().trim_matches('"');
    if trimmed == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(trimmed));
    }

    if let Some(rest) = trimmed
        .strip_prefix("~/")
        .or_else(|| trimmed.strip_prefix("~\\"))
    {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }

    PathBuf::from(trimmed)
}

pub(crate) fn normalize_import_source(source_path: Option<String>) -> Result<PathBuf, String> {
    let source = source_path
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(expand_home_path)
        .or_else(|| home_dir().map(|home| home.join(".ssh").join("config")))
        .ok_or_else(|| "Unable to locate the user home directory".to_string())?;

    let source = if source.is_dir() {
        source.join("config")
    } else {
        source
    };

    if !source.exists() {
        return Err(format!("SSH config file not found: {}", source.display()));
    }

    if !source.is_file() {
        return Err(format!(
            "SSH config path is not a file: {}",
            source.display()
        ));
    }

    Ok(source)
}

fn describe_ssh_config_read_error(path: &Path, error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::NotFound => {
            format!("SSH config file not found: {}", path.display())
        }
        std::io::ErrorKind::PermissionDenied => {
            format!(
                "Permission denied while reading SSH config file: {}",
                path.display()
            )
        }
        _ => format!(
            "Failed to read SSH config file '{}': {}",
            path.display(),
            error
        ),
    }
}

fn has_glob_pattern(value: &str) -> bool {
    value.contains('*') || value.contains('?')
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    fn matches(pattern: &[char], value: &[char]) -> bool {
        if pattern.is_empty() {
            return value.is_empty();
        }

        match pattern[0] {
            '*' => {
                matches(&pattern[1..], value)
                    || (!value.is_empty() && matches(pattern, &value[1..]))
            }
            '?' => !value.is_empty() && matches(&pattern[1..], &value[1..]),
            ch => !value.is_empty() && ch == value[0] && matches(&pattern[1..], &value[1..]),
        }
    }

    matches(
        &pattern.chars().collect::<Vec<_>>(),
        &value.chars().collect::<Vec<_>>(),
    )
}

fn expand_include_source(pattern: &str, base_dir: &Path) -> Vec<PathBuf> {
    let source = if pattern.starts_with('~') {
        expand_home_path(pattern)
    } else {
        let path = PathBuf::from(pattern);
        if path.is_absolute() {
            path
        } else {
            base_dir.join(path)
        }
    };

    if !has_glob_pattern(&source.to_string_lossy()) {
        return vec![source];
    }

    let mut candidates = vec![PathBuf::new()];
    for component in source.components() {
        match component {
            Component::Prefix(prefix) => {
                candidates = vec![PathBuf::from(prefix.as_os_str())];
            }
            Component::RootDir => {
                for candidate in &mut candidates {
                    candidate.push(component.as_os_str());
                }
            }
            Component::CurDir => {}
            Component::ParentDir => {
                for candidate in &mut candidates {
                    candidate.push("..");
                }
            }
            Component::Normal(part) => {
                let part = part.to_string_lossy().to_string();
                let mut next = Vec::new();
                if has_glob_pattern(&part) {
                    for candidate in &candidates {
                        if let Ok(entries) = fs::read_dir(candidate) {
                            for entry in entries.flatten() {
                                let name = entry.file_name().to_string_lossy().to_string();
                                if wildcard_matches(&part, &name) {
                                    next.push(entry.path());
                                }
                            }
                        }
                    }
                } else {
                    for mut candidate in candidates {
                        candidate.push(&part);
                        next.push(candidate);
                    }
                }
                candidates = next;
            }
        }
    }

    candidates.sort();
    candidates
}

pub(crate) fn read_ssh_config_with_includes(
    source: &Path,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) -> Result<(String, Vec<String>), String> {
    const MAX_INCLUDE_DEPTH: usize = 16;

    let canonical_source = source
        .canonicalize()
        .unwrap_or_else(|_| source.to_path_buf());
    if !visited.insert(canonical_source.clone()) {
        return Ok((
            String::new(),
            vec![format!(
                "Include loop skipped for SSH config file: {}",
                source.display()
            )],
        ));
    }

    if depth > MAX_INCLUDE_DEPTH {
        visited.remove(&canonical_source);
        return Ok((
            String::new(),
            vec![format!(
                "Include depth limit reached while reading SSH config file: {}",
                source.display()
            )],
        ));
    }

    let content =
        fs::read_to_string(source).map_err(|e| describe_ssh_config_read_error(source, &e))?;
    let base_dir = source.parent().unwrap_or_else(|| Path::new("."));
    let mut expanded = String::new();
    let mut warnings = Vec::new();

    for (line_index, raw_line) in content.lines().enumerate() {
        let line = strip_inline_comment(raw_line);
        let mut parts = split_ssh_words(&line);
        let is_include = parts
            .first()
            .map(|key| key.eq_ignore_ascii_case("include"))
            .unwrap_or(false);

        if !is_include {
            expanded.push_str(raw_line);
            expanded.push('\n');
            continue;
        }

        parts.remove(0);
        if parts.is_empty() {
            warnings.push(format!(
                "{}:{}: Include has no path and was ignored.",
                source.display(),
                line_index + 1
            ));
            continue;
        }

        for include_pattern in parts {
            let include_paths = expand_include_source(&include_pattern, base_dir);
            let readable_paths: Vec<PathBuf> = include_paths
                .into_iter()
                .filter(|path| path.is_file())
                .collect();

            if readable_paths.is_empty() {
                warnings.push(format!(
                    "{}:{}: Include '{}' did not match any readable files.",
                    source.display(),
                    line_index + 1,
                    include_pattern
                ));
                continue;
            }

            for include_path in readable_paths {
                match read_ssh_config_with_includes(&include_path, visited, depth + 1) {
                    Ok((included_content, mut include_warnings)) => {
                        expanded.push_str(&included_content);
                        warnings.append(&mut include_warnings);
                    }
                    Err(error) => warnings.push(error),
                }
            }
        }
    }

    visited.remove(&canonical_source);
    Ok((expanded, warnings))
}

fn strip_inline_comment(line: &str) -> String {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut output = String::new();

    for ch in line.chars() {
        if escaped {
            output.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' {
            output.push(ch);
            escaped = true;
            continue;
        }

        if ch == '\'' && !in_double {
            in_single = !in_single;
            output.push(ch);
            continue;
        }

        if ch == '"' && !in_single {
            in_double = !in_double;
            output.push(ch);
            continue;
        }

        if ch == '#' && !in_single && !in_double {
            break;
        }

        output.push(ch);
    }

    output.trim().to_string()
}

fn split_ssh_words(value: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    for ch in value.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if ch == '\'' && !in_double {
            in_single = !in_single;
            continue;
        }

        if ch == '"' && !in_single {
            in_double = !in_double;
            continue;
        }

        if ch.is_whitespace() && !in_single && !in_double {
            if !current.is_empty() {
                words.push(current.clone());
                current.clear();
            }
            continue;
        }

        current.push(ch);
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}

fn push_option(options: &mut HashMap<String, Vec<String>>, key: &str, value: String) {
    options
        .entry(key.to_ascii_lowercase())
        .or_default()
        .push(value);
}

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

fn is_wildcard_host(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?') || pattern.contains('!')
}

pub(crate) fn parse_ssh_config(
    content: &str,
) -> (Vec<RawSshHost>, SshConfigDefaults, Vec<String>) {
    let supported = HashSet::from([
        "host",
        "hostname",
        "user",
        "port",
        "identityfile",
        "proxyjump",
        "serveraliveinterval",
        "serveralivecountmax",
        "preferredauthentications",
        "identitiesonly",
    ]);
    let unsupported_tracked = HashSet::from([
        "proxycommand",
        "localforward",
        "remoteforward",
        "dynamicforward",
        "certificatefile",
        "controlmaster",
        "controlpath",
        "controlpersist",
        "requesttty",
        "remotecommand",
        "sendenv",
        "setenv",
        "forwardagent",
        "forwardx11",
        "hostkeyalias",
        "stricthostkeychecking",
        "userknownhostsfile",
    ]);

    let mut hosts = Vec::new();
    let mut defaults = SshConfigDefaults::default();
    let mut global_warnings = Vec::new();
    let mut current: Option<RawSshHost> = None;
    let mut skipping_match_block = false;

    for (line_index, raw_line) in content.lines().enumerate() {
        let line_no = line_index + 1;
        let line = strip_inline_comment(raw_line);
        if line.is_empty() {
            continue;
        }

        let mut parts = split_ssh_words(&line);
        if parts.is_empty() {
            continue;
        }

        let key = parts.remove(0).to_ascii_lowercase();
        let value = parts.join(" ");

        if key == "include" {
            global_warnings.push(format!("Line {line_no}: Include was not expanded."));
            continue;
        }

        if key == "match" {
            let criteria = if value.trim().is_empty() {
                "(empty criteria)".to_string()
            } else {
                value.trim().to_string()
            };
            global_warnings.push(format!(
                "Line {line_no}: Match block '{criteria}' is ignored during import."
            ));
            if let Some(host) = current.take() {
                hosts.push(host);
            }
            current = None;
            skipping_match_block = true;
            continue;
        }

        if key == "host" {
            skipping_match_block = false;
            if let Some(host) = current.take() {
                hosts.push(host);
            }

            let patterns = split_ssh_words(&value);
            if patterns.iter().any(|pattern| pattern == "*") {
                current = None;
            } else if let Some(pattern) = patterns.first() {
                current = Some(RawSshHost {
                    pattern: pattern.clone(),
                    ..Default::default()
                });

                if patterns.len() > 1 {
                    if let Some(host) = &mut current {
                        host.warnings.push(format!(
                            "Line {line_no}: multiple Host patterns found; only '{}' is imported.",
                            pattern
                        ));
                    }
                }
            }
            continue;
        }

        if skipping_match_block {
            continue;
        }

        if current.is_none() {
            match key.as_str() {
                "user" => defaults.user = Some(value),
                "port" => defaults.port = value.parse::<u16>().ok(),
                "identityfile" => defaults.identity_file = Some(value),
                "serveraliveinterval" => defaults.server_alive_interval = value.parse::<u32>().ok(),
                "serveralivecountmax" => {
                    defaults.server_alive_count_max = value.parse::<u32>().ok()
                }
                _ => {}
            }
            continue;
        }

        let Some(host) = &mut current else {
            continue;
        };

        if supported.contains(key.as_str()) {
            push_option(&mut host.options, &key, value);
        } else if unsupported_tracked.contains(key.as_str()) {
            if !host.unsupported_options.iter().any(|item| item == &key) {
                host.unsupported_options.push(key.clone());
            }
        }
    }

    if let Some(host) = current.take() {
        hosts.push(host);
    }

    (hosts, defaults, global_warnings)
}

fn parse_proxy_jump(
    proxy_jump: Option<String>,
    defaults: &SshConfigDefaults,
    private_key_path: Option<&str>,
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
                password: None,
            })
        })
        .collect()
}

pub(crate) fn build_import_host(
    raw: RawSshHost,
    defaults: &SshConfigDefaults,
    existing_profiles: &[super::types::SavedProfile],
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
        &mut warnings,
    );
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
        keepalive_interval_secs,
        keepalive_count_max,
        jump_hosts,
        warnings,
        unsupported_options: raw.unsupported_options,
        skipped,
        skip_reason,
        existing_profile_id,
    }
}

pub(crate) fn build_ssh_config_import_preview(
    source_path: Option<String>,
) -> Result<SshConfigImportPreview, String> {
    let source = normalize_import_source(source_path)?;
    let (content, include_warnings) =
        read_ssh_config_with_includes(&source, &mut HashSet::new(), 0)?;
    let (raw_hosts, defaults, mut warnings) = parse_ssh_config(&content);
    warnings.extend(include_warnings);
    let existing_profiles = load_profiles_from_disk()?;
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
mod tests {
    use super::{build_import_host, parse_ssh_config, read_ssh_config_with_includes};
    use crate::profiles::storage::sanitize_profile;
    use crate::profiles::types::SavedProfile;
    use std::collections::HashSet;
    use std::fs;

    #[test]
    fn sanitize_profile_migrates_legacy_jump_host_to_chain() {
        let mut profile: SavedProfile = serde_json::from_value(serde_json::json!({
            "id": "profile-1",
            "name": "demo",
            "group": "",
            "connection_type": "ssh",
            "remember_password": false,
            "keepalive_interval_secs": 30,
            "keepalive_count_max": 3,
            "server_monitor_visible": false,
            "jump_host": {
                "host": "bastion",
                "port": 22,
                "username": "stone",
                "auth_method": "password",
                "password": "secret"
            }
        }))
        .expect("profile should deserialize");

        sanitize_profile(&mut profile);

        assert_eq!(profile.jump_hosts.len(), 1);
        assert_eq!(profile.jump_hosts[0].host, "bastion");
        assert!(profile.jump_hosts[0].password.is_none());
        let serialized = serde_json::to_value(&profile).expect("profile should serialize");
        assert!(serialized.get("jump_host").is_none());
    }

    #[test]
    fn parse_ssh_config_maps_supported_host_fields() {
        let config = r#"
User default-user

Host prod
  HostName prod.example.com
  User deploy
  Port 2202
  IdentityFile ~/.ssh/prod_key
  ServerAliveInterval 15
  ServerAliveCountMax 4
  ProxyJump ops@bastion.example.com:2222
"#;

        let (raw_hosts, defaults, warnings) = parse_ssh_config(config);
        assert!(warnings.is_empty());
        assert_eq!(raw_hosts.len(), 1);

        let host = build_import_host(raw_hosts[0].clone(), &defaults, &[]);
        assert!(!host.skipped);
        assert_eq!(host.name, "prod");
        assert_eq!(host.host.as_deref(), Some("prod.example.com"));
        assert_eq!(host.username.as_deref(), Some("deploy"));
        assert_eq!(host.port, 2202);
        assert_eq!(host.auth_method, "key");
        assert_eq!(host.keepalive_interval_secs, 15);
        assert_eq!(host.keepalive_count_max, 4);
        assert_eq!(host.jump_hosts.len(), 1);
        assert_eq!(host.jump_hosts[0].host, "bastion.example.com");
        assert_eq!(host.jump_hosts[0].port, 2222);
        assert_eq!(host.jump_hosts[0].username, "ops");
        assert_eq!(host.jump_hosts[0].auth_method, "key");
        assert_eq!(
            host.jump_hosts[0].private_key_path.as_deref(),
            host.private_key_path.as_deref()
        );
        assert!(host
            .warnings
            .iter()
            .any(|warning| warning.contains("verify each jump host authentication setting")));
    }

    #[test]
    fn parse_ssh_config_skips_wildcard_hosts_and_tracks_unsupported_options() {
        let config = r#"
Host *.internal
  User deploy
  ProxyCommand ssh gateway nc %h %p
"#;

        let (raw_hosts, defaults, warnings) = parse_ssh_config(config);
        assert!(warnings.is_empty());
        assert_eq!(raw_hosts.len(), 1);

        let host = build_import_host(raw_hosts[0].clone(), &defaults, &[]);
        assert!(host.skipped);
        assert_eq!(
            host.skip_reason.as_deref(),
            Some("Wildcard Host patterns are not imported as saved profiles.")
        );
        assert_eq!(host.unsupported_options, vec!["proxycommand".to_string()]);
    }

    #[test]
    fn parse_ssh_config_ignores_match_block_options_until_next_host() {
        let config = r#"
User global-user

Match host prod
  User match-user
  IdentityFile ~/.ssh/match_key

Host prod
  HostName prod.example.com
"#;

        let (raw_hosts, defaults, warnings) = parse_ssh_config(config);
        assert_eq!(raw_hosts.len(), 1);
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("Match block 'host prod' is ignored")));

        let host = build_import_host(raw_hosts[0].clone(), &defaults, &[]);
        assert_eq!(host.username.as_deref(), Some("global-user"));
        assert_eq!(host.auth_method, "password");
        assert!(host.private_key_path.is_none());
    }

    #[test]
    fn read_ssh_config_expands_include_files() {
        let root =
            std::env::temp_dir().join(format!("tterm-ssh-config-include-{}", uuid::Uuid::new_v4()));
        let include_dir = root.join("config.d");
        fs::create_dir_all(&include_dir).expect("include dir should be created");
        let config_path = root.join("config");
        let included_path = include_dir.join("prod.conf");

        fs::write(
            &config_path,
            r#"
User default-user
Include config.d/*.conf
"#,
        )
        .expect("root config should be written");
        fs::write(
            &included_path,
            r#"
Host prod
  HostName prod.example.com
"#,
        )
        .expect("included config should be written");

        let (content, warnings) =
            read_ssh_config_with_includes(&config_path, &mut HashSet::new(), 0)
                .expect("include expansion should succeed");
        assert!(warnings.is_empty());
        assert!(content.contains("Host prod"));

        let (raw_hosts, defaults, parse_warnings) = parse_ssh_config(&content);
        assert!(parse_warnings.is_empty());
        let host = build_import_host(raw_hosts[0].clone(), &defaults, &[]);
        assert_eq!(host.username.as_deref(), Some("default-user"));
        assert_eq!(host.host.as_deref(), Some("prod.example.com"));

        let _ = fs::remove_dir_all(root);
    }
}
