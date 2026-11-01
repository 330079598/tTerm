//! Works out the options of each host from every block that matches it, the
//! way `ssh` reads its config: blocks apply in file order and the first value
//! of an option wins, so `Host *.corp` and `Host *` reach the hosts below and
//! above them alike. A `Match` block is checked against what the blocks above
//! it have set, and `Match final` makes a second pass that matches `Host`
//! lines against the `HostName`.

use super::matching::{criteria_match, MatchContext};
use super::parse::{BlockCondition, SshConfig, SshConfigBlock};
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

/// How a block matches a host.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockMatch {
    None,
    /// The line names the host itself.
    Named,
    /// A wildcard pattern on a `Host` line, or a `Match` line, matches.
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

fn push_once(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

/// What one host has gathered from the blocks read so far.
#[derive(Default)]
struct Resolution {
    options: HashMap<String, Vec<String>>,
    /// Gathered options from blocks that do not name the host itself.
    inherited: HashMap<String, Vec<String>>,
    unsupported_options: Vec<String>,
    warnings: Vec<String>,
    warned_rival: bool,
}

impl Resolution {
    fn first(&self, key: &str) -> Option<&str> {
        self.options
            .get(key)
            .and_then(|values| values.first())
            .map(String::as_str)
    }

    /// The `HostName` set so far, with `%h` filled in.
    fn hostname(&self, alias: &str) -> Option<String> {
        self.first("hostname")
            .map(|hostname| expand_hostname_tokens(hostname, alias))
    }

    fn apply(&mut self, block: &SshConfigBlock, matched: BlockMatch) {
        for (key, value) in &block.options {
            if GATHERED_OPTIONS.contains(&key.as_str()) {
                let gathered = if matched == BlockMatch::Named {
                    &mut self.options
                } else {
                    &mut self.inherited
                };
                // A second pass reads the same lines again.
                push_once(gathered.entry(key.clone()).or_default(), value);
                continue;
            }
            if self.options.contains_key(key) {
                continue;
            }
            if let Some(rival) =
                rival_proxy_option(key).filter(|rival| self.options.contains_key(*rival))
            {
                if !self.warned_rival {
                    self.warned_rival = true;
                    self.warnings.push(format!(
                        "{} was not imported because {} comes first, and ssh uses whichever comes first.",
                        option_label(key),
                        option_label(rival)
                    ));
                }
                continue;
            }
            self.options.insert(key.clone(), vec![value.clone()]);
        }
        for option in &block.unsupported_options {
            push_once(&mut self.unsupported_options, option);
        }
    }
}

/// Reads every block once for `alias`, matching `Host` lines against
/// `host_arg`: the alias on the first pass and the `HostName` on the final one.
fn read_blocks(
    config: &SshConfig,
    alias: &str,
    host_arg: &str,
    final_pass: bool,
    resolution: &mut Resolution,
) {
    for block in &config.blocks {
        let matched = match &block.condition {
            BlockCondition::Host(patterns) => match_block(patterns, host_arg),
            BlockCondition::Match(criteria) => {
                let host = resolution
                    .hostname(alias)
                    .unwrap_or_else(|| host_arg.to_string());
                let context = MatchContext {
                    host: &host,
                    original_host: alias,
                    user: resolution.first("user").unwrap_or(&config.local_user),
                    local_user: &config.local_user,
                    final_pass,
                };
                if criteria_match(criteria, &context) {
                    BlockMatch::Wildcard
                } else {
                    BlockMatch::None
                }
            }
            BlockCondition::Never => BlockMatch::None,
        };
        if matched != BlockMatch::None {
            resolution.apply(block, matched);
        }
    }
}

/// The options `ssh` would use for `alias`, as a raw host entry.
///
/// tTerm keeps one key file and one certificate where `ssh` tries each in
/// turn, so the ones the host's own blocks name come before those that
/// wildcard and `Match` blocks add.
pub(super) fn resolve_host(config: &SshConfig, alias: &str) -> RawSshHost {
    let mut resolution = Resolution::default();
    read_blocks(config, alias, alias, false, &mut resolution);
    if config.wants_final_pass {
        let host_arg = resolution
            .hostname(alias)
            .unwrap_or_else(|| alias.to_string());
        read_blocks(config, alias, &host_arg, true, &mut resolution);
    }

    let hostname = resolution.hostname(alias);
    let mut options = resolution.options;
    for (key, values) in resolution.inherited {
        let gathered = options.entry(key).or_default();
        for value in &values {
            push_once(gathered, value);
        }
    }
    if let Some(hostname) = hostname {
        options.insert("hostname".to_string(), vec![hostname]);
    }
    RawSshHost {
        pattern: alias.to_string(),
        options,
        unsupported_options: resolution.unsupported_options,
        warnings: resolution.warnings,
    }
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
/// gets no entry, and so does a `Match` line.
pub(super) fn import_entries(config: &SshConfig) -> Vec<RawSshHost> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for block in config.blocks.iter().filter(|block| block.line > 0) {
        let BlockCondition::Host(patterns) = &block.condition else {
            continue;
        };
        if patterns.iter().any(|pattern| pattern == "*") {
            continue;
        }
        let Some(pattern) = patterns.first() else {
            continue;
        };
        if !seen.insert(pattern.clone()) {
            continue;
        }
        let mut entry = if is_wildcard_host(pattern) {
            wildcard_entry(block, pattern)
        } else {
            resolve_host(config, pattern)
        };
        if patterns.len() > 1 {
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
