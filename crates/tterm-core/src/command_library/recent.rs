//! The recent-command list and the draft a command is saved from, from
//! `src/lib/recentCommands.ts`.
//!
//! The WebView still owns the `localStorage` read and write; these functions
//! take the stored text and return the value to persist.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::js::{is_js_whitespace, trim_js, utf16_len, utf16_slice};

/// The `localStorage` key the WebView keeps the list under.
pub const RECENT_COMMANDS_STORAGE_KEY: &str = "tterm.recent-commands.v1";

/// The number of commands kept; the least recently used are forgotten first.
pub const RECENT_COMMANDS_LIMIT: usize = 100;

/// A saved command's draft before it is stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandDraft {
    /// The name suggested from the command text.
    pub name: String,
    /// The command text without surrounding whitespace.
    pub command_text: String,
    /// An optional description; empty for a generated draft.
    pub description: String,
    /// Tags to save the command under.
    pub tags: Vec<String>,
    /// `"global"` or `"profile"`.
    pub scope_type: String,
    /// The profile the command belongs to, when scoped to one.
    pub scope_id: Option<String>,
    /// Whether the command is a favorite; generated drafts are.
    pub is_favorite: bool,
}

/// A command as the recent list keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentCommand {
    /// The id the command was saved under while it stays recent.
    pub id: String,
    /// The command text without surrounding whitespace.
    pub command_text: String,
    /// The profile the command ran in.
    pub profile_id: Option<String>,
    /// The profile's name when it ran.
    pub profile_name: Option<String>,
    /// Unix milliseconds of the last run.
    pub last_used_at: i64,
    /// How often the command ran.
    pub use_count: i64,
}

/// One command a terminal ran, for the recent list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedCommand {
    /// The command text as it was run.
    pub command_text: String,
    /// The profile the command ran in.
    pub profile_id: Option<String>,
    /// The profile's name when it ran.
    pub profile_name: Option<String>,
    /// Unix milliseconds of the run.
    pub executed_at: i64,
}

fn command_key(command_text: &str, profile_id: Option<&str>) -> String {
    format!(
        "{}\u{0}{}",
        profile_id.unwrap_or("global"),
        trim_js(command_text)
    )
}

/// The name suggested for a command: its first non-blank line, shortened.
pub fn suggest_command_name(command_text: &str) -> String {
    let Some(first_line) = command_text
        .split('\n')
        .map(trim_js)
        .find(|line| !line.is_empty())
    else {
        return String::new();
    };

    if utf16_len(first_line) <= 48 {
        return first_line.to_owned();
    }
    let truncated = utf16_slice(first_line, 0, 45);
    let shortened = trim_end_js(&truncated);
    format!("{shortened}...")
}

fn trim_end_js(value: &str) -> &str {
    value.trim_end_matches(is_js_whitespace)
}

/// The draft to save a command from, scoped to `profile_id` when it ran there.
pub fn create_command_draft(command_text: &str, profile_id: Option<&str>) -> CommandDraft {
    let normalized = trim_js(command_text);
    CommandDraft {
        name: suggest_command_name(normalized),
        command_text: normalized.to_owned(),
        description: String::new(),
        tags: Vec::new(),
        scope_type: if profile_id.is_some() {
            "profile".to_owned()
        } else {
            "global".to_owned()
        },
        scope_id: profile_id.map(str::to_owned),
        is_favorite: true,
    }
}

/// Records a run, moving the same command in the same profile to the front.
pub fn add_recent_command(
    commands: &[RecentCommand],
    executed: &ExecutedCommand,
) -> Vec<RecentCommand> {
    let command_text = trim_js(&executed.command_text);
    if command_text.is_empty() {
        return commands.to_vec();
    }

    let key = command_key(command_text, executed.profile_id.as_deref());
    let existing = commands
        .iter()
        .find(|command| command_key(&command.command_text, command.profile_id.as_deref()) == key);
    let recent = RecentCommand {
        id: existing.map_or_else(
            || format!("{}:{}", executed.executed_at, key),
            |command| command.id.clone(),
        ),
        command_text: command_text.to_owned(),
        profile_id: executed.profile_id.clone(),
        profile_name: executed.profile_name.clone(),
        last_used_at: executed.executed_at,
        use_count: existing.map_or(1, |command| command.use_count + 1),
    };

    let mut next = vec![recent];
    next.extend(
        commands
            .iter()
            .filter(|command| Some(&command.id) != existing.map(|found| &found.id))
            .cloned(),
    );
    next.truncate(RECENT_COMMANDS_LIMIT);
    next
}

/// Reads the stored list, keeping only the well-formed entries.
pub fn parse_recent_commands(raw: Option<&str>) -> Vec<RecentCommand> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    let Value::Array(items) = parsed else {
        return Vec::new();
    };

    items
        .into_iter()
        .filter_map(|item| {
            let Value::Object(object) = item else {
                return None;
            };
            let id = object.get("id")?.as_str()?.to_owned();
            let command_text = object.get("commandText")?.as_str()?.to_owned();
            let last_used_at = object.get("lastUsedAt")?.as_i64()?;
            let use_count = object.get("useCount")?.as_i64()?;
            Some(RecentCommand {
                id,
                command_text,
                profile_id: object
                    .get("profileId")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                profile_name: object
                    .get("profileName")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                last_used_at,
                use_count,
            })
        })
        .take(RECENT_COMMANDS_LIMIT)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        CommandDraft, ExecutedCommand, add_recent_command, create_command_draft,
        suggest_command_name,
    };

    fn executed(command_text: &str, profile_id: Option<&str>, executed_at: i64) -> ExecutedCommand {
        ExecutedCommand {
            command_text: command_text.to_owned(),
            profile_id: profile_id.map(str::to_owned),
            profile_name: None,
            executed_at,
        }
    }

    #[test]
    fn deduplicates_the_same_command_within_a_profile() {
        let first = add_recent_command(&[], &executed("  git status ", Some("one"), 10));
        let second = add_recent_command(&first, &executed("git status", Some("one"), 20));

        assert_eq!(second.len(), 1);
        assert_eq!(second[0].command_text, "git status");
        assert_eq!(second[0].last_used_at, 20);
        assert_eq!(second[0].use_count, 2);
    }

    #[test]
    fn keeps_identical_commands_from_different_profiles_separate() {
        let commands = add_recent_command(
            &add_recent_command(&[], &executed("uptime", Some("one"), 10)),
            &executed("uptime", Some("two"), 20),
        );
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn creates_a_favorite_draft_scoped_to_the_current_profile() {
        assert_eq!(
            create_command_draft("  docker ps  ", Some("p1")),
            CommandDraft {
                name: "docker ps".to_owned(),
                command_text: "docker ps".to_owned(),
                description: String::new(),
                tags: Vec::new(),
                scope_type: "profile".to_owned(),
                scope_id: Some("p1".to_owned()),
                is_favorite: true,
            }
        );
    }

    #[test]
    fn truncates_generated_names() {
        let name = suggest_command_name(&"x".repeat(60));
        assert_eq!(name.chars().count(), 48);
        assert!(name.ends_with("..."));
    }
}
