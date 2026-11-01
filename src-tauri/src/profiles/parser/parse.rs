//! Turns SSH config text into its `Host` and `Match` blocks.

use super::matching::{local_user_name, parse_match_criteria, MatchCriterion};
use super::words::{split_keyword, split_ssh_words, strip_inline_comment};
use std::collections::HashSet;

/// When a block applies.
#[derive(Debug, Clone)]
pub(super) enum BlockCondition {
    /// The patterns on a `Host` line. Lines above the first `Host` or `Match`
    /// apply to every host, as `*`.
    Host(Vec<String>),
    /// The criteria on a `Match` line, all of which must hold.
    Match(Vec<MatchCriterion>),
    /// A `Match` line tTerm cannot check, whose block never applies.
    Never,
}

/// The lines under one `Host` or `Match` line, or above the first one.
#[derive(Debug, Clone)]
pub(super) struct SshConfigBlock {
    pub condition: BlockCondition,
    /// The line of the `Host` or `Match` keyword, or 0 for the lines above
    /// the first one.
    pub line: usize,
    /// The supported options with lowercase keys, in the order they appear.
    pub options: Vec<(String, String)>,
    /// The tracked options tTerm does not support, each once.
    pub unsupported_options: Vec<String>,
}

impl SshConfigBlock {
    fn new(condition: BlockCondition, line: usize) -> Self {
        Self {
            condition,
            line,
            options: Vec::new(),
            unsupported_options: Vec::new(),
        }
    }
}

/// An SSH config file, read into blocks.
#[derive(Debug, Clone)]
pub(super) struct SshConfig {
    /// The blocks in file order.
    pub blocks: Vec<SshConfigBlock>,
    /// Whether a `Match final` asks `ssh` to read the blocks a second time.
    pub wants_final_pass: bool,
    /// The user running tTerm, for `Match localuser`.
    pub local_user: String,
}

/// Splits SSH config text into its blocks in file order, with warnings about
/// the lines that were left out.
pub(super) fn parse_ssh_config(content: &str) -> (SshConfig, Vec<String>) {
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

    let mut blocks = vec![SshConfigBlock::new(
        BlockCondition::Host(vec!["*".to_string()]),
        0,
    )];
    let mut warnings = Vec::new();
    let mut wants_final_pass = false;

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
            let condition = match parse_match_criteria(&split_ssh_words(rest)) {
                Ok(criteria) => {
                    wants_final_pass |= criteria.iter().any(MatchCriterion::requests_final_pass);
                    BlockCondition::Match(criteria)
                }
                Err(reason) => {
                    warnings.push(format!(
                        "Line {line_no}: {reason}, so the block under 'Match {}' is not applied.",
                        value.trim()
                    ));
                    BlockCondition::Never
                }
            };
            blocks.push(SshConfigBlock::new(condition, line_no));
            continue;
        }

        if key == "host" {
            blocks.push(SshConfigBlock::new(
                BlockCondition::Host(split_ssh_words(&value)),
                line_no,
            ));
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

    let config = SshConfig {
        blocks,
        wants_final_pass,
        local_user: local_user_name(),
    };
    (config, warnings)
}
