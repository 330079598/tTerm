use crate::config::{ensure_config_dir, get_config_path};
use crate::ssh::{SecretLocation, SshClientHandler};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn default_auth_method() -> String {
    "password".to_string()
}

/// Jump host configuration stored as part of a saved profile.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SavedJumpHost {
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default = "default_auth_method")]
    pub auth_method: String,
    #[serde(default)]
    pub private_key_path: Option<String>,
    #[serde(default, skip_serializing)]
    pub private_key_passphrase: Option<String>,
    #[serde(default, skip_serializing)]
    pub password: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavedProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub group: String,
    pub connection_type: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    #[serde(default, skip_serializing)]
    pub password: Option<String>,
    #[serde(default, skip_serializing)]
    pub ignore_saved_password: bool,
    #[serde(default)]
    pub remember_password: bool,
    pub auth_method: Option<String>,
    pub private_key_path: Option<String>,
    #[serde(default, skip_serializing)]
    pub private_key_passphrase: Option<String>,
    #[serde(default = "default_keepalive_interval")]
    pub keepalive_interval_secs: u32,
    #[serde(default = "default_keepalive_count")]
    pub keepalive_count_max: u32,
    #[serde(default)]
    pub server_monitor_visible: bool,
    /// Legacy single jump host field kept only for backward-compatible reads.
    #[serde(default, rename = "jump_host", skip_serializing)]
    legacy_jump_host: Option<SavedJumpHost>,
    /// Ordered jump host chain used throughout the app and for all new saves.
    #[serde(default)]
    pub jump_hosts: Vec<SavedJumpHost>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SavedSecretSummary {
    pub key: String,
    pub profile_id: String,
    pub profile_name: String,
    pub label: String,
    pub kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigImportOptions {
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub overwrite_existing: bool,
    #[serde(default)]
    pub selected_hosts: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigImportHost {
    pub host_pattern: String,
    pub name: String,
    pub host: Option<String>,
    pub port: u16,
    pub username: Option<String>,
    pub auth_method: String,
    pub private_key_path: Option<String>,
    pub keepalive_interval_secs: u32,
    pub keepalive_count_max: u32,
    pub jump_hosts: Vec<SavedJumpHost>,
    pub warnings: Vec<String>,
    pub unsupported_options: Vec<String>,
    pub skipped: bool,
    pub skip_reason: Option<String>,
    pub existing_profile_id: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigImportPreview {
    pub source_path: String,
    pub hosts: Vec<SshConfigImportHost>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigImportResult {
    pub imported: usize,
    pub updated: usize,
    pub skipped: usize,
    pub profiles: Vec<SavedProfile>,
}

#[derive(Debug, Default, Clone)]
struct SshConfigDefaults {
    user: Option<String>,
    port: Option<u16>,
    identity_file: Option<String>,
    server_alive_interval: Option<u32>,
    server_alive_count_max: Option<u32>,
}

#[derive(Debug, Default, Clone)]
struct RawSshHost {
    pattern: String,
    options: HashMap<String, Vec<String>>,
    unsupported_options: Vec<String>,
    warnings: Vec<String>,
}

fn default_keepalive_interval() -> u32 {
    30
}
fn default_keepalive_count() -> u32 {
    3
}

fn load_profiles_from_disk() -> Result<Vec<SavedProfile>, String> {
    let config_dir = get_config_path()?;
    let profiles_file = config_dir.join("profiles.json");
    if !profiles_file.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(&profiles_file)
        .map_err(|e| format!("Failed to read profiles file: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse profiles: {}", e))
}

fn normalize_profile(profile: &mut SavedProfile) {
    if profile.jump_hosts.is_empty() {
        if let Some(jump) = profile.legacy_jump_host.take() {
            profile.jump_hosts.push(jump);
        }
    } else {
        profile.legacy_jump_host = None;
    }

    if profile.group.trim().is_empty() {
        profile.group = String::new();
    }
}

fn sanitize_profile(profile: &mut SavedProfile) {
    normalize_profile(profile);
    profile.password = None;
    profile.ignore_saved_password = false;
    profile.private_key_passphrase = None;
    if let Some(jump) = &mut profile.legacy_jump_host {
        jump.password = None;
        jump.private_key_passphrase = None;
    }
    for jump in &mut profile.jump_hosts {
        jump.password = None;
        jump.private_key_passphrase = None;
    }
}

fn home_dir() -> Option<PathBuf> {
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

fn expand_home_path(path: &str) -> PathBuf {
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

fn normalize_import_source(source_path: Option<String>) -> Result<PathBuf, String> {
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

fn read_ssh_config_with_includes(
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

fn parse_ssh_config(content: &str) -> (Vec<RawSshHost>, SshConfigDefaults, Vec<String>) {
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

fn build_import_host(
    raw: RawSshHost,
    defaults: &SshConfigDefaults,
    existing_profiles: &[SavedProfile],
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

fn build_ssh_config_import_preview(
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

fn write_profiles_to_disk(profiles: &[SavedProfile]) -> Result<(), String> {
    let config_dir = ensure_config_dir()?;
    let profiles_file = config_dir.join("profiles.json");
    let content = serde_json::to_string_pretty(profiles)
        .map_err(|e| format!("Failed to serialize profiles: {}", e))?;
    fs::write(&profiles_file, content).map_err(|e| format!("Failed to write profiles file: {}", e))
}

fn profile_secret_summaries(profile: &SavedProfile) -> Vec<SavedSecretSummary> {
    let mut summaries = Vec::new();

    if profile.connection_type == "ssh" && profile.auth_method.as_deref() != Some("key") {
        summaries.push(SavedSecretSummary {
            key: profile.id.clone(),
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
            label: profile.name.clone(),
            kind: "ssh".to_string(),
        });
        if !profile.name.trim().is_empty() {
            summaries.push(SavedSecretSummary {
                key: profile.name.clone(),
                profile_id: profile.id.clone(),
                profile_name: profile.name.clone(),
                label: profile.name.clone(),
                kind: "ssh-legacy-name".to_string(),
            });
        }
    }

    for jump in &profile.jump_hosts {
        if jump.auth_method == "key" {
            continue;
        }

        let label = format!(
            "{} via {}@{}:{}",
            profile.name, jump.username, jump.host, jump.port
        );
        summaries.push(SavedSecretSummary {
            key: crate::core::session::jump_host_identity_secret_key(
                Some(profile.id.as_str()),
                profile.name.as_str(),
                &jump.host,
                jump.port,
                &jump.username,
            ),
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
            label: label.clone(),
            kind: "jump-host".to_string(),
        });
        if !profile.name.trim().is_empty() {
            summaries.push(SavedSecretSummary {
                key: crate::core::session::jump_host_identity_secret_key(
                    None,
                    profile.name.as_str(),
                    &jump.host,
                    jump.port,
                    &jump.username,
                ),
                profile_id: profile.id.clone(),
                profile_name: profile.name.clone(),
                label,
                kind: "jump-host-legacy-name".to_string(),
            });
        }
    }

    summaries
}

pub fn saved_secret_summaries() -> Result<Vec<SavedSecretSummary>, String> {
    let profiles = load_profiles_from_disk()?;
    let mut summaries = Vec::new();

    for mut profile in profiles {
        normalize_profile(&mut profile);
        summaries.extend(profile_secret_summaries(&profile));
    }

    Ok(summaries)
}

pub fn saved_secret_keys() -> Result<Vec<String>, String> {
    Ok(saved_secret_summaries()?
        .into_iter()
        .map(|summary| summary.key)
        .collect())
}

fn delete_profile_secrets(
    app: &tauri::AppHandle,
    secret_state: &crate::ssh::SecretStoreState,
    profile: &SavedProfile,
) -> Result<usize, String> {
    let mut deleted = 0;
    for summary in profile_secret_summaries(profile) {
        if secret_state.delete_password(app, &summary.key)? {
            deleted += 1;
        }
    }
    Ok(deleted)
}

fn profile_groups_file_path() -> Result<std::path::PathBuf, String> {
    Ok(get_config_path()?.join("profile_groups.json"))
}

fn normalize_group_name(value: &str) -> String {
    value.trim().to_string()
}

fn push_unique_group(groups: &mut Vec<String>, group: String) {
    if group.is_empty() || groups.iter().any(|existing| existing == &group) {
        return;
    }

    groups.push(group);
}

fn load_configured_profile_groups() -> Result<Vec<String>, String> {
    let groups_file = profile_groups_file_path()?;
    if !groups_file.exists() {
        return Ok(vec![]);
    }

    let content = fs::read_to_string(&groups_file)
        .map_err(|e| format!("Failed to read profile groups file: {}", e))?;
    let raw_groups: Vec<String> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse profile groups: {}", e))?;

    let mut groups = Vec::new();
    for group in raw_groups {
        push_unique_group(&mut groups, normalize_group_name(&group));
    }

    Ok(groups)
}

fn write_profile_groups_to_disk(groups: &[String]) -> Result<(), String> {
    let config_dir = ensure_config_dir()?;
    let groups_file = config_dir.join("profile_groups.json");
    let mut normalized_groups = Vec::new();
    for group in groups {
        push_unique_group(&mut normalized_groups, normalize_group_name(group));
    }
    normalized_groups.sort_by(|left, right| left.to_lowercase().cmp(&right.to_lowercase()));

    let content = serde_json::to_string_pretty(&normalized_groups)
        .map_err(|e| format!("Failed to serialize profile groups: {}", e))?;
    fs::write(&groups_file, content)
        .map_err(|e| format!("Failed to write profile groups file: {}", e))
}

fn load_all_profile_groups() -> Result<Vec<String>, String> {
    let mut groups = load_configured_profile_groups()?;
    for profile in load_profiles_from_disk()? {
        push_unique_group(&mut groups, normalize_group_name(&profile.group));
    }
    groups.sort_by(|left, right| left.to_lowercase().cmp(&right.to_lowercase()));
    Ok(groups)
}

#[tauri::command]
pub fn list_profiles() -> Result<Vec<SavedProfile>, String> {
    let mut profiles = load_profiles_from_disk()?;
    for profile in &mut profiles {
        sanitize_profile(profile);
    }
    Ok(profiles)
}

#[tauri::command]
pub fn preview_ssh_config_import(
    source_path: Option<String>,
) -> Result<SshConfigImportPreview, String> {
    build_ssh_config_import_preview(source_path)
}

#[tauri::command]
pub fn import_ssh_config_profiles(
    options: SshConfigImportOptions,
) -> Result<SshConfigImportResult, String> {
    let preview = build_ssh_config_import_preview(options.source_path)?;
    let selected_hosts = options
        .selected_hosts
        .into_iter()
        .collect::<HashSet<String>>();
    let import_all = selected_hosts.is_empty();
    let group = normalize_group_name(
        options
            .group
            .as_deref()
            .unwrap_or("Imported from SSH config"),
    );
    let mut profiles = load_profiles_from_disk()?;
    let mut imported = 0;
    let mut updated = 0;
    let mut skipped = 0;

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
            .position(|profile| profile.name == host.name && profile.group == group);

        if existing_index.is_some() && !options.overwrite_existing {
            skipped += 1;
            continue;
        }

        let profile = SavedProfile {
            id: existing_index
                .map(|index| profiles[index].id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name: host.name,
            group: group.clone(),
            connection_type: "ssh".to_string(),
            host: Some(hostname),
            port: Some(host.port),
            username: host.username,
            password: None,
            ignore_saved_password: false,
            remember_password: false,
            auth_method: Some(host.auth_method),
            private_key_path: host.private_key_path,
            private_key_passphrase: None,
            keepalive_interval_secs: host.keepalive_interval_secs,
            keepalive_count_max: host.keepalive_count_max,
            server_monitor_visible: false,
            legacy_jump_host: None,
            jump_hosts: host.jump_hosts,
        };

        if let Some(index) = existing_index {
            profiles[index] = profile;
            updated += 1;
        } else {
            profiles.push(profile);
            imported += 1;
        }
    }

    for profile in &mut profiles {
        sanitize_profile(profile);
    }
    write_profiles_to_disk(&profiles)?;

    if !group.is_empty() {
        let mut groups = load_configured_profile_groups()?;
        push_unique_group(&mut groups, group);
        write_profile_groups_to_disk(&groups)?;
    }

    let mut sanitized_profiles = profiles.clone();
    for profile in &mut sanitized_profiles {
        sanitize_profile(profile);
    }

    Ok(SshConfigImportResult {
        imported,
        updated,
        skipped,
        profiles: sanitized_profiles,
    })
}

#[tauri::command]
pub fn save_profile(
    app: tauri::AppHandle,
    secret_state: tauri::State<'_, crate::ssh::SecretStoreState>,
    mut profile: SavedProfile,
) -> Result<(), String> {
    normalize_profile(&mut profile);

    if profile.auth_method.as_deref() != Some("key") && profile.remember_password {
        if let Some(password) = profile
            .password
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            let location = secret_state.save_password(&app, profile.id.as_str(), password)?;
            if matches!(location, SecretLocation::Memory) {
                return Err(
                    "Password persistence is unavailable. Enable the app vault or use a supported system credential store."
                        .to_string(),
                );
            }
        }
    }

    if !profile.remember_password || profile.auth_method.as_deref() == Some("key") {
        let _ = secret_state.delete_password(&app, profile.id.as_str());
        if !profile.name.trim().is_empty() {
            let _ = secret_state.delete_password(&app, profile.name.as_str());
        }
    }

    for jump in &profile.jump_hosts {
        if profile.remember_password && jump.auth_method != "key" {
            if let Some(password) = jump.password.as_deref().filter(|value| !value.is_empty()) {
                let secret_key = crate::core::session::jump_host_identity_secret_key(
                    Some(profile.id.as_str()),
                    profile.name.as_str(),
                    &jump.host,
                    jump.port,
                    &jump.username,
                );
                let location = secret_state.save_password(&app, &secret_key, password)?;
                if matches!(location, SecretLocation::Memory) {
                    return Err(
                        "Jump host password persistence is unavailable. Enable the app vault or use a supported system credential store."
                            .to_string(),
                    );
                }
            }
        } else {
            let secret_key = crate::core::session::jump_host_identity_secret_key(
                Some(profile.id.as_str()),
                profile.name.as_str(),
                &jump.host,
                jump.port,
                &jump.username,
            );
            let _ = secret_state.delete_password(&app, &secret_key);
            if !profile.name.trim().is_empty() {
                let name_key = crate::core::session::jump_host_identity_secret_key(
                    None,
                    profile.name.as_str(),
                    &jump.host,
                    jump.port,
                    &jump.username,
                );
                let _ = secret_state.delete_password(&app, &name_key);
            }
        }
    }

    sanitize_profile(&mut profile);

    let mut profiles = load_profiles_from_disk()?;
    if let Some(pos) = profiles.iter().position(|p| p.id == profile.id) {
        let mut previous = profiles[pos].clone();
        normalize_profile(&mut previous);
        for summary in profile_secret_summaries(&previous) {
            if !profile_secret_summaries(&profile)
                .iter()
                .any(|current| current.key == summary.key)
            {
                let _ = secret_state.delete_password(&app, &summary.key);
            }
        }
        profiles[pos] = profile;
    } else {
        profiles.push(profile);
    }
    for existing in &mut profiles {
        sanitize_profile(existing);
    }
    write_profiles_to_disk(&profiles)
}

#[tauri::command]
pub fn list_profile_groups() -> Result<Vec<String>, String> {
    load_all_profile_groups()
}

#[tauri::command]
pub fn save_profile_group(name: String) -> Result<Vec<String>, String> {
    let name = normalize_group_name(&name);
    if name.is_empty() {
        return Err("Group name is required".to_string());
    }

    let mut groups = load_all_profile_groups()?;
    push_unique_group(&mut groups, name);
    write_profile_groups_to_disk(&groups)?;
    load_all_profile_groups()
}

#[tauri::command]
pub fn rename_profile_group(old_name: String, new_name: String) -> Result<Vec<String>, String> {
    let old_name = normalize_group_name(&old_name);
    let new_name = normalize_group_name(&new_name);
    if old_name.is_empty() || new_name.is_empty() {
        return Err("Group name is required".to_string());
    }

    let mut profiles = load_profiles_from_disk()?;
    let mut profiles_changed = false;
    for profile in &mut profiles {
        if normalize_group_name(&profile.group) == old_name {
            profile.group = new_name.clone();
            profiles_changed = true;
        }
    }
    if profiles_changed {
        write_profiles_to_disk(&profiles)?;
    }

    let mut groups = load_configured_profile_groups()?
        .into_iter()
        .filter(|group| normalize_group_name(group) != old_name)
        .collect::<Vec<_>>();
    push_unique_group(&mut groups, new_name);
    write_profile_groups_to_disk(&groups)?;
    load_all_profile_groups()
}

#[tauri::command]
pub fn delete_profile_group(name: String) -> Result<Vec<String>, String> {
    let name = normalize_group_name(&name);
    if name.is_empty() {
        return Err("Group name is required".to_string());
    }

    let mut profiles = load_profiles_from_disk()?;
    let mut profiles_changed = false;
    for profile in &mut profiles {
        if normalize_group_name(&profile.group) == name {
            profile.group = String::new();
            profiles_changed = true;
        }
    }
    if profiles_changed {
        write_profiles_to_disk(&profiles)?;
    }

    let groups = load_configured_profile_groups()?
        .into_iter()
        .filter(|group| normalize_group_name(group) != name)
        .collect::<Vec<_>>();
    write_profile_groups_to_disk(&groups)?;
    load_all_profile_groups()
}

#[tauri::command]
pub fn move_profile_to_group(
    id: String,
    group: String,
    target_id: Option<String>,
) -> Result<(), String> {
    let group = normalize_group_name(&group);
    let mut profiles = load_profiles_from_disk()?;
    let Some(source_index) = profiles.iter().position(|profile| profile.id == id) else {
        return Err("Profile not found".to_string());
    };

    let mut profile = profiles.remove(source_index);
    profile.group = group.clone();
    let insert_index = target_id
        .as_deref()
        .and_then(|target_id| profiles.iter().position(|profile| profile.id == target_id))
        .unwrap_or(profiles.len());
    profiles.insert(insert_index, profile);
    write_profiles_to_disk(&profiles)?;

    if !group.is_empty() {
        let mut groups = load_configured_profile_groups()?;
        push_unique_group(&mut groups, group);
        write_profile_groups_to_disk(&groups)?;
    }

    Ok(())
}

#[tauri::command]
pub fn delete_profile(
    app: tauri::AppHandle,
    secret_state: tauri::State<'_, crate::ssh::SecretStoreState>,
    id: String,
) -> Result<(), String> {
    let mut profiles = load_profiles_from_disk()?;
    if let Some(profile) = profiles.iter().find(|p| p.id == id).cloned() {
        let mut profile = profile;
        normalize_profile(&mut profile);
        delete_profile_secrets(&app, &secret_state, &profile)?;
    }
    profiles.retain(|p| p.id != id);
    write_profiles_to_disk(&profiles)
}

#[tauri::command]
pub fn set_profile_server_monitor_visible(id: String, visible: bool) -> Result<(), String> {
    let mut profiles = load_profiles_from_disk()?;
    if let Some(profile) = profiles.iter_mut().find(|profile| profile.id == id) {
        profile.server_monitor_visible = visible;
        write_profiles_to_disk(&profiles)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn test_connection(
    app: tauri::AppHandle,
    profile: SavedProfile,
    prompt_state: tauri::State<'_, crate::core::state::HostPromptMap>,
    secret_state: tauri::State<'_, crate::ssh::SecretStoreState>,
) -> Result<String, String> {
    if profile.connection_type != "ssh" {
        return Err("Only SSH connections can be tested".to_string());
    }

    let host = profile.host.clone().ok_or("Host is required")?;
    let username = profile.username.clone().ok_or("Username is required")?;
    let port = profile.port.unwrap_or(22);

    let mut profile = profile;
    normalize_profile(&mut profile);

    let jump_hosts = profile
        .jump_hosts
        .into_iter()
        .map(|j| crate::core::session::JumpHostPlan {
            host: j.host,
            port: j.port,
            username: j.username,
            password: j.password,
            private_key_path: if j.auth_method == "key" {
                j.private_key_path
            } else {
                None
            },
            private_key_passphrase: if j.auth_method == "key" {
                j.private_key_passphrase
            } else {
                None
            },
        })
        .collect::<Vec<_>>();

    let mut plan = crate::core::session::SessionPlan {
        kind: crate::core::SessionKind::Ssh,
        profile_id: Some(profile.id.clone()),
        profile_name: profile.name.clone(),
        host: Some(host.clone()),
        port,
        username: Some(username.clone()),
        password: profile.password.clone(),
        ignore_saved_password: profile.ignore_saved_password,
        remember_password: false,
        private_key_path: if profile.auth_method.as_deref() == Some("key") {
            profile.private_key_path.clone()
        } else {
            None
        },
        private_key_passphrase: profile.private_key_passphrase.clone(),
        terminal_shell: None,
        keepalive_interval_secs: profile.keepalive_interval_secs as u16,
        keepalive_count_max: profile.keepalive_count_max as u16,
        jump_hosts,
    };

    let test_tab_id = format!("test-{}", profile.id);
    crate::core::session::resolve_ssh_password(&app, &secret_state, &mut plan)?;

    crate::ssh::emit_connection_progress(
        &app,
        &test_tab_id,
        crate::ssh::ConnectionStatusOptions::SILENT,
        crate::ssh::SshConnectionProgressPayload::new(
            "resolving_credentials",
            "Resolved saved credentials for test connection",
        ),
    );

    // Try to establish connection
    use std::time::Duration;

    if !plan.jump_hosts.is_empty() {
        let target_config = std::sync::Arc::new(crate::ssh::jump::compatibility_client_config(
            plan.keepalive_interval_secs as u64,
            plan.keepalive_count_max as usize,
        ));

        let (jump_chain, mut target_session) = crate::ssh::jump::connect_via_jump_chain(
            &app,
            &test_tab_id,
            &plan.jump_hosts,
            &host,
            port,
            test_connection_handler(
                &app,
                &test_tab_id,
                &plan,
                &host,
                port,
                prompt_state.inner().clone(),
            ),
            target_config,
            prompt_state.inner().clone(),
            crate::ssh::ConnectionStatusOptions::SILENT,
        )
        .await?;

        // Authenticate on target through tunnel
        crate::ssh::emit_connection_progress(
            &app,
            &test_tab_id,
            crate::ssh::ConnectionStatusOptions::SILENT,
            crate::ssh::SshConnectionProgressPayload::new(
                "target_authenticating",
                format!("Authenticating target as {}", username),
            )
            .host(host.clone(), port)
            .username(username.clone()),
        );

        let auth_result =
            authenticate_test_connection(&mut target_session, &username, &plan).await?;

        match auth_result {
            russh::client::AuthResult::Success => {
                crate::ssh::emit_connection_progress(
                    &app,
                    &test_tab_id,
                    crate::ssh::ConnectionStatusOptions::SILENT,
                    crate::ssh::SshConnectionProgressPayload::new(
                        "ready",
                        format!("Successfully connected to {}@{}:{}", username, host, port),
                    )
                    .host(host.clone(), port)
                    .username(username.clone()),
                );
            }
            _ => return Err("Authentication failed".to_string()),
        }

        let _ = target_session
            .disconnect(russh::Disconnect::ByApplication, "", "")
            .await;
        drop(jump_chain);
    } else {
        // Direct connection
        use russh::client;

        let config = std::sync::Arc::new(crate::ssh::jump::compatibility_client_config(
            plan.keepalive_interval_secs as u64,
            plan.keepalive_count_max as usize,
        ));

        crate::ssh::emit_connection_progress(
            &app,
            &test_tab_id,
            crate::ssh::ConnectionStatusOptions::SILENT,
            crate::ssh::SshConnectionProgressPayload::new(
                "target_connecting",
                format!("Connecting to target {}:{}", host, port),
            )
            .host(host.clone(), port)
            .username(username.clone()),
        );

        let mut session = tokio::time::timeout(
            Duration::from_secs(10),
            client::connect(
                config,
                (host.as_str(), port),
                test_connection_handler(
                    &app,
                    &test_tab_id,
                    &plan,
                    &host,
                    port,
                    prompt_state.inner().clone(),
                ),
            ),
        )
        .await
        .map_err(|_| "Connection timeout".to_string())?
        .map_err(|e| format!("Connection failed: {}", e))?;

        crate::ssh::emit_connection_progress(
            &app,
            &test_tab_id,
            crate::ssh::ConnectionStatusOptions::SILENT,
            crate::ssh::SshConnectionProgressPayload::new(
                "target_authenticating",
                format!("Authenticating target as {}", username),
            )
            .host(host.clone(), port)
            .username(username.clone()),
        );

        let auth_result = authenticate_test_connection(&mut session, &username, &plan).await?;

        match auth_result {
            russh::client::AuthResult::Success => {
                crate::ssh::emit_connection_progress(
                    &app,
                    &test_tab_id,
                    crate::ssh::ConnectionStatusOptions::SILENT,
                    crate::ssh::SshConnectionProgressPayload::new(
                        "ready",
                        format!("Successfully connected to {}@{}:{}", username, host, port),
                    )
                    .host(host.clone(), port)
                    .username(username.clone()),
                );
            }
            _ => return Err("Authentication failed".to_string()),
        }

        session
            .disconnect(russh::Disconnect::ByApplication, "", "")
            .await
            .ok();
    }

    Ok(format!(
        "Successfully connected to {}@{}:{}",
        username, host, port
    ))
}

fn test_connection_handler(
    app: &tauri::AppHandle,
    tab_id: &str,
    plan: &crate::core::session::SessionPlan,
    host: &str,
    port: u16,
    prompts: crate::core::state::HostPromptMap,
) -> SshClientHandler {
    SshClientHandler {
        app: app.clone(),
        tab_id: tab_id.to_string(),
        profile_id: plan.profile_id.clone(),
        profile_name: plan.profile_name.clone(),
        host: host.to_string(),
        port,
        prompts,
        user_rejected_host_key: Arc::new(AtomicBool::new(false)),
        status_options: crate::ssh::ConnectionStatusOptions::SILENT,
    }
}

async fn authenticate_test_connection<H: russh::client::Handler>(
    session: &mut russh::client::Handle<H>,
    username: &str,
    plan: &crate::core::session::SessionPlan,
) -> Result<russh::client::AuthResult, String> {
    if let Some(key_path) = &plan.private_key_path {
        let key_data = std::fs::read_to_string(key_path)
            .map_err(|e| format!("Failed to read private key: {}", e))?;

        let key = if let Some(passphrase) = &plan.private_key_passphrase {
            russh::keys::decode_secret_key(&key_data, Some(passphrase))
                .map_err(|e| format!("Failed to decode private key: {}", e))?
        } else {
            russh::keys::decode_secret_key(&key_data, None)
                .map_err(|e| format!("Failed to decode private key: {}", e))?
        };

        session
            .authenticate_publickey(
                username,
                russh::keys::PrivateKeyWithHashAlg::new(std::sync::Arc::new(key), None),
            )
            .await
            .map_err(|e| format!("Authentication failed: {}", e))
    } else {
        let password = plan.password.as_deref().ok_or("Password is required")?;
        session
            .authenticate_password(username, password)
            .await
            .map_err(|e| format!("Authentication failed: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_import_host, parse_ssh_config, read_ssh_config_with_includes, sanitize_profile,
        SavedProfile,
    };
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
