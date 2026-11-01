//! Reads the criteria of `Match` lines and checks them for a host, the way
//! `ssh` does in `readconf.c`.

use super::pattern::wildcard_matches;

/// What one `Match` criterion compares.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Criterion {
    All,
    /// `canonical` and `final`, which hold on the second pass only.
    FinalPass,
    Host(String),
    OriginalHost(String),
    User(String),
    LocalUser(String),
    SessionType(String),
}

/// One criterion of a `Match` line, which `!` negates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MatchCriterion {
    negated: bool,
    criterion: Criterion,
}

impl MatchCriterion {
    /// Whether this is a plain `final`, which asks `ssh` for a second pass.
    pub(super) fn requests_final_pass(&self) -> bool {
        !self.negated && self.criterion == Criterion::FinalPass
    }
}

/// What a host looks like when a `Match` line is checked against it.
pub(super) struct MatchContext<'a> {
    /// The `HostName` set so far, or the name being looked up.
    pub host: &'a str,
    /// The name as it was typed.
    pub original_host: &'a str,
    /// The `User` set so far, or the local user.
    pub user: &'a str,
    pub local_user: &'a str,
    pub final_pass: bool,
}

/// Matches a comma-separated list of patterns, where a matching `!pattern`
/// rules the value out.
fn pattern_list_matches(list: &str, value: &str, ignore_case: bool) -> bool {
    let fold = |text: &str| {
        if ignore_case {
            text.to_ascii_lowercase()
        } else {
            text.to_string()
        }
    };
    let value = fold(value);
    let mut matched = false;
    for pattern in list.split(',').map(fold) {
        if let Some(negated) = pattern.strip_prefix('!') {
            if wildcard_matches(negated, &value) {
                return false;
            }
        } else if wildcard_matches(&pattern, &value) {
            matched = true;
        }
    }
    matched
}

/// Reads the words after `Match`. A criterion tTerm cannot check during an
/// import, such as `exec`, which would run a command, gives the reason why
/// the block is not applied.
pub(super) fn parse_match_criteria(words: &[String]) -> Result<Vec<MatchCriterion>, String> {
    let mut criteria = Vec::new();
    let mut words = words.iter();
    while let Some(word) = words.next() {
        let lower = word.to_ascii_lowercase();
        let (negated, name) = match lower.strip_prefix('!') {
            Some(name) => (true, name.to_string()),
            None => (false, lower.clone()),
        };
        let criterion = match name.as_str() {
            "all" => Criterion::All,
            "canonical" | "final" => Criterion::FinalPass,
            "host" | "originalhost" | "user" | "localuser" | "sessiontype" => {
                let Some(value) = words.next() else {
                    return Err(format!("Match {name} has no value"));
                };
                let value = value.clone();
                match name.as_str() {
                    "host" => Criterion::Host(value),
                    "originalhost" => Criterion::OriginalHost(value),
                    "user" => Criterion::User(value),
                    "localuser" => Criterion::LocalUser(value),
                    _ => Criterion::SessionType(value),
                }
            }
            "exec" | "localnetwork" | "tagged" | "command" | "version" => {
                return Err(format!("Match {name} cannot be checked during import"));
            }
            _ => return Err(format!("Match criterion '{word}' is not known")),
        };
        criteria.push(MatchCriterion { negated, criterion });
    }
    if criteria.is_empty() {
        return Err("Match has no criteria".to_string());
    }
    Ok(criteria)
}

/// Whether every criterion of a `Match` line holds for the host.
pub(super) fn criteria_match(criteria: &[MatchCriterion], context: &MatchContext) -> bool {
    criteria.iter().all(|item| {
        let holds = match &item.criterion {
            Criterion::All => true,
            Criterion::FinalPass => context.final_pass,
            Criterion::Host(list) => pattern_list_matches(list, context.host, true),
            Criterion::OriginalHost(list) => {
                pattern_list_matches(list, context.original_host, true)
            }
            Criterion::User(list) => pattern_list_matches(list, context.user, false),
            Criterion::LocalUser(list) => pattern_list_matches(list, context.local_user, false),
            // tTerm opens an interactive shell.
            Criterion::SessionType(list) => pattern_list_matches(list, "shell", true),
        };
        holds != item.negated
    })
}

/// The name of the user running tTerm, which `Match localuser` compares.
pub(super) fn local_user_name() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default()
}
