//! Works out the options of each host from every block that matches it, the
//! way `ssh` reads its config: blocks apply in file order and the first value
//! of an option wins, so `Host *.corp` and `Host *` reach the hosts below and
//! above them alike.

use super::parse::SshConfigBlock;
use super::pattern::{is_wildcard_host, wildcard_matches};
use crate::profiles::types::RawSshHost;
use std::collections::{HashMap, HashSet};

/// Options `ssh` gathers from every matching block instead of keeping the
/// first value.
const GATHERED_OPTIONS: [&str; 5] = [
    "identityfile",
    "certificatefile",
    "localforward",
    "remoteforward",
    "dynamicforward",
];

/// How a block's `Host` line matches a host name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockMatch {
    None,
    /// The line names the host itself.
    Named,
    /// Only a wildcard pattern on the line matches.
    Wildcard,
}

/// Matches a host name against a `Host` line: a negated `!pattern` that
/// matches rules the block out, and `ssh` compares host names without case.
fn match_block(patterns: &[String], alias: &str) -> BlockMatch {
    let alias = alias.to_ascii_lowercase();
    let mut result = BlockMatch::None;
    for pattern in patterns {
        let pattern = pattern.to_ascii_lowercase();
        if let Some(negated) = pattern.strip_prefix('!') {
            if wildcard_matches(negated, &alias) {
                return BlockMatch::None;
            }
        } else if pattern == alias {
            result = BlockMatch::Named;
        } else if result == BlockMatch::None && wildcard_matches(&pattern, &alias) {
            result = BlockMatch::Wildcard;
        }
    }
    result
}

/// `ProxyJump` and `ProxyCommand` compete: whichever comes first stops the
/// other, as the `ssh_config` manual says.
fn rival_proxy_option(key: &str) -> Option<&'static str> {
    match key {
        "proxyjump" => Some("proxycommand"),
        "proxycommand" => Some("proxyjump"),
        _ => None,
    }
}

fn option_label(key: &str) -> &'static str {
    if key == "proxyjump" {
        "ProxyJump"
    } else {
        "ProxyCommand"
    }
}

/// `HostName` may name the host as it was typed with `%h`, and `%%` is a `%`.
fn expand_hostname_tokens(value: &str, alias: &str) -> String {
    let mut expanded = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            expanded.push(ch);
            continue;
        }
        match chars.next() {
            Some('h') => expanded.push_str(alias),
            Some('%') => expanded.push('%'),
            Some(other) => {
                expanded.push('%');
                expanded.push(other);
            }
            None => expanded.push('%'),
        }
    }
    expanded
}

/// The options `ssh` would use for `alias`, as a raw host entry.
///
/// tTerm keeps one key file and one certificate where `ssh` tries each in
/// turn, so the ones the host's own blocks name come before those that
/// wildcard blocks add.
pub(super) fn resolve_host(blocks: &[SshConfigBlock], alias: &str) -> RawSshHost {
    let mut options: HashMap<String, Vec<String>> = HashMap::new();
    let mut inherited: HashMap<String, Vec<String>> = HashMap::new();
    let mut host = RawSshHost {
        pattern: alias.to_string(),
        ..Default::default()
    };
    let mut warned_rival = false;

    for block in blocks {
        let matched = match_block(&block.patterns, alias);
        if matched == BlockMatch::None {
            continue;
        }
        for (key, value) in &block.options {
            if GATHERED_OPTIONS.contains(&key.as_str()) {
                let gathered = if matched == BlockMatch::Named {
                    &mut options
                } else {
                    &mut inherited
                };
                gathered.entry(key.clone()).or_default().push(value.clone());
                continue;
            }
            if options.contains_key(key) {
                continue;
            }
            if let Some(rival) =
                rival_proxy_option(key).filter(|rival| options.contains_key(*rival))
            {
                if !warned_rival {
                    warned_rival = true;
                    host.warnings.push(format!(
                        "{} was not imported because {} comes first, and ssh uses whichever comes first.",
                        option_label(key),
                        option_label(rival)
                    ));
                }
                continue;
            }
            options.insert(key.clone(), vec![value.clone()]);
        }
        for option in &block.unsupported_options {
            if !host.unsupported_options.contains(option) {
                host.unsupported_options.push(option.clone());
            }
        }
    }

    for (key, values) in inherited {
        options.entry(key).or_default().extend(values);
    }
    if let Some(hostname) = options
        .get_mut("hostname")
        .and_then(|values| values.first_mut())
    {
        *hostname = expand_hostname_tokens(hostname, alias);
    }
    host.options = options;
    host
}

/// A wildcard block's own options, for the preview entry that shows it was
/// not imported.
fn wildcard_entry(block: &SshConfigBlock, pattern: &str) -> RawSshHost {
    let mut options: HashMap<String, Vec<String>> = HashMap::new();
    for (key, value) in &block.options {
        options.entry(key.clone()).or_default().push(value.clone());
    }
    RawSshHost {
        pattern: pattern.to_string(),
        options,
        unsupported_options: block.unsupported_options.clone(),
        warnings: Vec::new(),
    }
}

/// One entry per name that starts a `Host` line, in the order the names first
/// appear. A line with `*` among its patterns sets options for other hosts and
/// gets no entry.
pub(super) fn import_entries(blocks: &[SshConfigBlock]) -> Vec<RawSshHost> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for block in blocks.iter().filter(|block| block.line > 0) {
        if block.patterns.iter().any(|pattern| pattern == "*") {
            continue;
        }
        let Some(pattern) = block.patterns.first() else {
            continue;
        };
        if !seen.insert(pattern.clone()) {
            continue;
        }
        let mut entry = if is_wildcard_host(pattern) {
            wildcard_entry(block, pattern)
        } else {
            resolve_host(blocks, pattern)
        };
        if block.patterns.len() > 1 {
            entry.warnings.insert(
                0,
                format!(
                    "Line {}: multiple Host patterns found; only '{pattern}' is imported.",
                    block.line
                ),
            );
        }
        entries.push(entry);
    }
    entries
}
