//! Turns SSH config text into its `Host` blocks.

use super::words::{split_keyword, split_ssh_words, strip_inline_comment};
use std::collections::HashSet;

/// The lines under one `Host` line, or above the first one.
#[derive(Debug, Default, Clone)]
pub(super) struct SshConfigBlock {
    /// The patterns on the `Host` line. Lines above the first `Host` apply to
    /// every host, as `*`.
    pub patterns: Vec<String>,
    /// The line of the `Host` keyword, or 0 for the lines above the first one.
    pub line: usize,
    /// The supported options with lowercase keys, in the order they appear.
    pub options: Vec<(String, String)>,
    /// The tracked options tTerm does not support, each once.
    pub unsupported_options: Vec<String>,
}

/// Splits SSH config text into its blocks in file order, with warnings about
/// the lines that were left out.
pub(super) fn parse_ssh_config(content: &str) -> (Vec<SshConfigBlock>, Vec<String>) {
    let supported = HashSet::from([
        "host",
        "hostname",
        "user",
        "port",
        "identityfile",
        "certificatefile",
        "proxyjump",
        "proxycommand",
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

    let mut blocks = vec![SshConfigBlock {
        patterns: vec!["*".to_string()],
        ..Default::default()
    }];
    let mut warnings = Vec::new();
    let mut skipping_match_block = false;

    for (line_index, raw_line) in content.lines().enumerate() {
        let line_no = line_index + 1;
        let line = strip_inline_comment(raw_line);
        if line.is_empty() {
            continue;
        }

        let (key, rest) = split_keyword(&line);
        let key = key.to_ascii_lowercase();
        // A shell runs the proxy command, so its quotes are kept for the shell.
        let value = if key == "proxycommand" {
            rest.to_string()
        } else {
            split_ssh_words(rest).join(" ")
        };

        if key == "include" {
            warnings.push(format!("Line {line_no}: Include was not expanded."));
            continue;
        }

        if key == "match" {
            let criteria = if value.trim().is_empty() {
                "(empty criteria)".to_string()
            } else {
                value.trim().to_string()
            };
            warnings.push(format!(
                "Line {line_no}: Match block '{criteria}' is ignored during import."
            ));
            skipping_match_block = true;
            continue;
        }

        if key == "host" {
            skipping_match_block = false;
            blocks.push(SshConfigBlock {
                patterns: split_ssh_words(&value),
                line: line_no,
                ..Default::default()
            });
            continue;
        }

        if skipping_match_block {
            continue;
        }
        let Some(block) = blocks.last_mut() else {
            continue;
        };

        if supported.contains(key.as_str()) {
            block.options.push((key, value));
        } else if unsupported_tracked.contains(key.as_str())
            && !block.unsupported_options.contains(&key)
        {
            block.unsupported_options.push(key);
        }
    }

    (blocks, warnings)
}
