//! Turns SSH config text into its `Host` and `Match` blocks, reading the
//! files its `Include` lines name in place.

use super::include::IncludeReader;
use super::matching::{local_user_name, parse_match_criteria, MatchCriterion};
use super::words::{split_keyword, split_ssh_words, strip_inline_comment};
use std::mem;

/// The options tTerm imports.
const SUPPORTED_OPTIONS: [&str; 15] = [
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
];

/// The options tTerm does not support but tells the user about.
const TRACKED_UNSUPPORTED_OPTIONS: [&str; 11] = [
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
];

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

fn every_host() -> BlockCondition {
    BlockCondition::Host(vec!["*".to_string()])
}

/// The lines under one `Host` or `Match` line, or above the first one.
#[derive(Debug, Clone)]
pub(super) struct SshConfigBlock {
    pub condition: BlockCondition,
    /// The conditions of the blocks that hold the `Include` this block was
    /// read through, outermost first. They must hold too, as `ssh` reads an
    /// `Include` under a `Host` or `Match` line only for the hosts it matches.
    pub guards: Vec<BlockCondition>,
    /// Whether a `Host` or `Match` line starts the block, rather than the top
    /// of a file or the end of an `Include` that resumes the block above it.
    pub has_header: bool,
    /// Where the block starts, as warnings name it.
    pub place: String,
    /// The supported options with lowercase keys, in the order they appear.
    pub options: Vec<(String, String)>,
    /// The tracked options tTerm does not support, each once.
    pub unsupported_options: Vec<String>,
}

impl SshConfigBlock {
    fn new(condition: BlockCondition, guards: &[BlockCondition], place: String) -> Self {
        Self {
            condition,
            guards: guards.to_vec(),
            has_header: true,
            place,
            options: Vec::new(),
            unsupported_options: Vec::new(),
        }
    }

    /// The same block again after an `Include`, which may have started
    /// blocks of its own.
    fn resumed(&self) -> Self {
        Self {
            has_header: false,
            options: Vec::new(),
            unsupported_options: Vec::new(),
            ..self.clone()
        }
    }

    fn is_empty(&self) -> bool {
        self.options.is_empty() && self.unsupported_options.is_empty()
    }
}

/// An SSH config file, read into blocks.
#[derive(Debug, Clone)]
pub(super) struct SshConfig {
    /// The blocks in file order, with included files read in place.
    pub blocks: Vec<SshConfigBlock>,
    /// Whether a `Match final` asks `ssh` to read the blocks a second time.
    pub wants_final_pass: bool,
    /// The user running tTerm, for `Match localuser`.
    pub local_user: String,
}

/// Splits SSH config text into its blocks in file order, with warnings about
/// the lines that were left out. `Include` lines are not followed.
#[cfg(test)]
pub(super) fn parse_ssh_config(content: &str) -> (SshConfig, Vec<String>) {
    parse_ssh_config_with(content, None)
}

/// Splits SSH config text into its blocks in file order, reading the files
/// its `Include` lines name through `reader`.
pub(super) fn parse_ssh_config_with(
    content: &str,
    reader: Option<&mut IncludeReader>,
) -> (SshConfig, Vec<String>) {
    let mut parser = Parser {
        reader,
        blocks: Vec::new(),
        warnings: Vec::new(),
        wants_final_pass: false,
    };
    parser.parse_file(content, None, &[]);
    let config = SshConfig {
        blocks: parser.blocks,
        wants_final_pass: parser.wants_final_pass,
        local_user: local_user_name(),
    };
    (config, parser.warnings)
}

struct Parser<'r> {
    reader: Option<&'r mut IncludeReader>,
    blocks: Vec<SshConfigBlock>,
    warnings: Vec<String>,
    wants_final_pass: bool,
}

/// How warnings name a line: by number in the file being imported, and with
/// the file's path in an included one.
fn place(file: Option<&str>, line_no: usize) -> String {
    match file {
        Some(file) => format!("{file}:{line_no}"),
        None => format!("Line {line_no}"),
    }
}

impl Parser<'_> {
    /// Reads one file into blocks under `guards`.
    fn parse_file(&mut self, content: &str, file: Option<&str>, guards: &[BlockCondition]) {
        let mut block = SshConfigBlock::new(every_host(), guards, place(file, 1)).resumed();

        for (line_index, raw_line) in content.lines().enumerate() {
            let line = strip_inline_comment(raw_line);
            if line.is_empty() {
                continue;
            }
            let here = place(file, line_index + 1);
            let (key, rest) = split_keyword(&line);
            let key = key.to_ascii_lowercase();

            match key.as_str() {
                "include" => {
                    let resumed = block.resumed();
                    let mut inner_guards = guards.to_vec();
                    inner_guards.push(block.condition.clone());
                    self.push(mem::replace(&mut block, resumed));
                    self.include(rest, &here, &inner_guards);
                }
                "match" => {
                    let condition = self.match_condition(rest, &here);
                    self.push(mem::replace(
                        &mut block,
                        SshConfigBlock::new(condition, guards, here),
                    ));
                }
                "host" => {
                    let condition = BlockCondition::Host(split_ssh_words(rest));
                    self.push(mem::replace(
                        &mut block,
                        SshConfigBlock::new(condition, guards, here),
                    ));
                }
                _ if SUPPORTED_OPTIONS.contains(&key.as_str()) => {
                    // A shell runs the proxy command, so its quotes are kept for the shell.
                    let value = if key == "proxycommand" {
                        rest.to_string()
                    } else {
                        split_ssh_words(rest).join(" ")
                    };
                    block.options.push((key, value));
                }
                _ if TRACKED_UNSUPPORTED_OPTIONS.contains(&key.as_str())
                    && !block.unsupported_options.contains(&key) =>
                {
                    block.unsupported_options.push(key);
                }
                _ => {}
            }
        }
        self.push(block);
    }

    /// Keeps a finished block, leaving out the empty parts of a file that
    /// only an `Include` or the file's top started.
    fn push(&mut self, block: SshConfigBlock) {
        if block.has_header || !block.is_empty() {
            self.blocks.push(block);
        }
    }

    fn match_condition(&mut self, rest: &str, here: &str) -> BlockCondition {
        match parse_match_criteria(&split_ssh_words(rest)) {
            Ok(criteria) => {
                self.wants_final_pass |= criteria.iter().any(MatchCriterion::requests_final_pass);
                BlockCondition::Match(criteria)
            }
            Err(reason) => {
                self.warnings.push(format!(
                    "{here}: {reason}, so the block under 'Match {}' is not applied.",
                    split_ssh_words(rest).join(" ")
                ));
                BlockCondition::Never
            }
        }
    }

    /// Reads the files an `Include` line names in place, each under `guards`.
    fn include(&mut self, rest: &str, here: &str, guards: &[BlockCondition]) {
        let patterns = split_ssh_words(rest);
        let Some(reader) = self.reader.as_deref() else {
            self.warnings
                .push(format!("{here}: Include was not expanded."));
            return;
        };
        if patterns.is_empty() {
            self.warnings
                .push(format!("{here}: Include has no path and was ignored."));
            return;
        }
        let mut found = Vec::new();
        for pattern in patterns {
            let paths = reader.files(&pattern);
            if paths.is_empty() {
                self.warnings.push(format!(
                    "{here}: Include '{pattern}' did not match any readable files in {}.",
                    reader.base_dir().display()
                ));
            }
            found.extend(paths);
        }
        for path in found {
            let Some(reader) = self.reader.as_deref_mut() else {
                return;
            };
            match reader.enter(&path) {
                Ok(content) => {
                    self.parse_file(&content, Some(&path.display().to_string()), guards);
                    if let Some(reader) = self.reader.as_deref_mut() {
                        reader.leave();
                    }
                }
                Err(warning) => self.warnings.push(warning),
            }
        }
    }
}
