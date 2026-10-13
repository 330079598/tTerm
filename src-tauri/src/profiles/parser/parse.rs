//! Turns SSH config text into raw `Host` blocks and the global defaults.

use super::words::{split_ssh_words, strip_inline_comment};
use crate::profiles::types::{RawSshHost, SshConfigDefaults};
use std::collections::{HashMap, HashSet};

fn push_option(options: &mut HashMap<String, Vec<String>>, key: &str, value: String) {
    options
        .entry(key.to_ascii_lowercase())
        .or_default()
        .push(value);
}

pub(super) fn parse_ssh_config(content: &str) -> (Vec<RawSshHost>, SshConfigDefaults, Vec<String>) {
    let supported = HashSet::from([
        "host",
        "hostname",
        "user",
        "port",
        "identityfile",
        "certificatefile",
        "proxyjump",
        "serveraliveinterval",
        "serveralivecountmax",
        "preferredauthentications",
        "identitiesonly",
        "forwardagent",
        "localforward",
        "remoteforward",
        "dynamicforward",
    ]);
    let unsupported_tracked = HashSet::from([
        "proxycommand",
        "controlmaster",
        "controlpath",
        "controlpersist",
        "requesttty",
        "remotecommand",
        "sendenv",
        "setenv",
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
                "certificatefile" => defaults.certificate_file = Some(value),
                "serveraliveinterval" => defaults.server_alive_interval = value.parse::<u32>().ok(),
                "serveralivecountmax" => {
                    defaults.server_alive_count_max = value.parse::<u32>().ok()
                }
                "forwardagent" => defaults.forward_agent = Some(value),
                _ => {}
            }
            continue;
        }

        let Some(host) = &mut current else {
            continue;
        };

        if supported.contains(key.as_str()) {
            push_option(&mut host.options, &key, value);
        } else if unsupported_tracked.contains(key.as_str())
            && !host.unsupported_options.iter().any(|item| item == &key)
        {
            host.unsupported_options.push(key.clone());
        }
    }

    if let Some(host) = current.take() {
        hosts.push(host);
    }

    (hosts, defaults, global_warnings)
}
