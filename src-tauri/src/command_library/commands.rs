use super::{CommandRepository, CommandVariable, SavedCommand};
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCommandInput {
    pub id: Option<String>,
    pub name: String,
    pub command_text: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_scope_type")]
    pub scope_type: String,
    pub scope_id: Option<String>,
    #[serde(default)]
    pub is_favorite: bool,
    /// `None` keeps the stored value, so callers that predate the field do not reset it.
    #[serde(default)]
    pub confirm_before_run: Option<bool>,
    /// Placeholders (`{{name}}`) to ask for on insert. `None` keeps the stored ones.
    #[serde(default)]
    pub variables: Option<Vec<CommandVariable>>,
}

fn default_scope_type() -> String {
    "global".to_string()
}

#[tauri::command]
pub fn list_saved_commands() -> Result<Vec<SavedCommand>, String> {
    CommandRepository::new(crate::db::get()?).list()
}

#[tauri::command]
pub fn list_command_tags() -> Result<Vec<String>, String> {
    CommandRepository::new(crate::db::get()?).list_tags()
}

#[tauri::command]
pub fn create_command_tag(tag: String) -> Result<String, String> {
    CommandRepository::new(crate::db::get()?).create_tag(&tag)
}

#[tauri::command]
pub fn rename_command_tag(old_tag: String, new_tag: String) -> Result<Vec<String>, String> {
    CommandRepository::new(crate::db::get()?).rename_tag(&old_tag, &new_tag)
}

#[tauri::command]
pub fn delete_command_tag(tag: String) -> Result<Vec<String>, String> {
    CommandRepository::new(crate::db::get()?).delete_tag(&tag)
}

#[tauri::command]
pub fn save_saved_command(input: SaveCommandInput) -> Result<SavedCommand, String> {
    save(
        &CommandRepository::new(crate::db::get()?),
        input,
        Utc::now().timestamp_millis(),
    )
}

#[tauri::command]
pub fn delete_saved_command(id: String) -> Result<bool, String> {
    CommandRepository::new(crate::db::get()?).delete(id.trim())
}

#[tauri::command]
pub fn set_saved_command_favorite(id: String, favorite: bool) -> Result<SavedCommand, String> {
    let repository = CommandRepository::new(crate::db::get()?);
    let mut command = repository
        .get(id.trim())?
        .ok_or_else(|| "Saved command not found".to_string())?;
    command.is_favorite = favorite;
    command.updated_at = Utc::now().timestamp_millis();
    repository.save(&command)?;
    Ok(command)
}

#[tauri::command]
pub fn record_saved_command_use(id: String) -> Result<(), String> {
    let updated = CommandRepository::new(crate::db::get()?)
        .record_use(id.trim(), Utc::now().timestamp_millis())?;
    if updated {
        Ok(())
    } else {
        Err("Saved command not found".to_string())
    }
}

fn save(
    repository: &CommandRepository<'_>,
    input: SaveCommandInput,
    now: i64,
) -> Result<SavedCommand, String> {
    let normalized_id = input
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let existing = match normalized_id {
        Some(id) => Some(
            repository
                .get(id)?
                .ok_or_else(|| "Saved command not found".to_string())?,
        ),
        None => None,
    };
    let tags = normalize_tags(input.tags);
    let variables = input.variables.map(normalize_variables).transpose()?;

    let command = SavedCommand {
        id: existing
            .as_ref()
            .map(|command| command.id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        name: input.name.trim().to_string(),
        command_text: input.command_text,
        description: input.description.trim().to_string(),
        scope_type: input.scope_type.trim().to_string(),
        scope_id: input
            .scope_id
            .map(|scope_id| scope_id.trim().to_string())
            .filter(|scope_id| !scope_id.is_empty()),
        shell_type: existing
            .as_ref()
            .map(|command| command.shell_type.clone())
            .unwrap_or_else(|| "any".to_string()),
        platform: existing
            .as_ref()
            .map(|command| command.platform.clone())
            .unwrap_or_else(|| "any".to_string()),
        is_favorite: input.is_favorite,
        confirm_before_run: input.confirm_before_run.unwrap_or_else(|| {
            existing
                .as_ref()
                .is_some_and(|command| command.confirm_before_run)
        }),
        sort_order: existing.as_ref().map_or(0, |command| command.sort_order),
        use_count: existing.as_ref().map_or(0, |command| command.use_count),
        last_used_at: existing.as_ref().and_then(|command| command.last_used_at),
        created_at: existing.as_ref().map_or(now, |command| command.created_at),
        updated_at: now,
        tags,
        variables: variables.unwrap_or_else(|| {
            existing
                .map(|command| command.variables)
                .unwrap_or_default()
        }),
    };

    repository.save(&command)?;
    Ok(command)
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    tags.into_iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .filter(|tag| seen.insert(tag.to_lowercase()))
        .collect()
}

/// A placeholder name as it may appear between `{{` and `}}`.
fn is_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Trims the definitions, numbers them in the given order and drops the
/// fields that do not apply to their type.
fn normalize_variables(variables: Vec<CommandVariable>) -> Result<Vec<CommandVariable>, String> {
    let mut seen = HashSet::new();
    variables
        .into_iter()
        .enumerate()
        .map(|(index, variable)| {
            let name = variable.name.trim().to_string();
            if !is_variable_name(&name) {
                return Err(format!(
                    "Command variable name '{name}' must start with a letter or underscore \
                     and contain only letters, digits and underscores"
                ));
            }
            if !seen.insert(name.clone()) {
                return Err(format!("Command variable '{name}' is defined twice"));
            }

            let options = if variable.value_type == "choice" {
                let options = parse_choice_options(variable.options_json.as_deref())
                    .ok_or_else(|| format!("Command variable '{name}' has invalid options"))?;
                if options.is_empty() {
                    return Err(format!(
                        "Command variable '{name}' needs at least one option"
                    ));
                }
                Some(options)
            } else {
                None
            };
            let default_value = variable
                .default_value
                .filter(|value| !value.is_empty())
                .filter(|value| match (variable.value_type.as_str(), &options) {
                    ("secret", _) => false,
                    (_, Some(options)) => options.contains(value),
                    _ => true,
                });
            if variable.value_type == "number" {
                if let Some(value) = &default_value {
                    if value.trim().parse::<f64>().is_err() {
                        return Err(format!(
                            "Command variable '{name}' has a default that is not a number"
                        ));
                    }
                }
            }

            let label = variable.label.trim();
            Ok(CommandVariable {
                label: if label.is_empty() {
                    name.clone()
                } else {
                    label.to_string()
                },
                name,
                value_type: variable.value_type,
                default_value,
                options_json: options
                    .map(|options| serde_json::to_string(&options))
                    .transpose()
                    .map_err(|error| format!("Failed to encode variable options: {error}"))?,
                is_required: variable.is_required,
                position: index as i64,
            })
        })
        .collect()
}

/// The distinct, non-empty options of a `choice` variable; `None` when the
/// stored value is not a JSON array of strings.
fn parse_choice_options(options_json: Option<&str>) -> Option<Vec<String>> {
    let options: Vec<String> = serde_json::from_str(options_json?).ok()?;
    let mut seen = HashSet::new();
    Some(
        options
            .into_iter()
            .map(|option| option.trim().to_string())
            .filter(|option| !option.is_empty())
            .filter(|option| seen.insert(option.clone()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_library::CommandVariable;
    use crate::db::Database;

    fn create_input() -> SaveCommandInput {
        SaveCommandInput {
            id: None,
            name: "  Docker logs  ".to_string(),
            command_text: "docker logs -f api".to_string(),
            description: "  Follow logs  ".to_string(),
            tags: vec![
                " Docker ".to_string(),
                "docker".to_string(),
                "logs".to_string(),
            ],
            scope_type: "global".to_string(),
            scope_id: None,
            is_favorite: true,
            confirm_before_run: None,
            variables: None,
        }
    }

    fn variable(name: &str, value_type: &str) -> CommandVariable {
        CommandVariable {
            name: name.to_string(),
            label: String::new(),
            value_type: value_type.to_string(),
            default_value: None,
            options_json: None,
            is_required: true,
            position: 99,
        }
    }

    #[test]
    fn creates_and_normalizes_user_input() {
        let database = Database::open_in_memory().expect("open database");
        let repository = CommandRepository::new(&database);

        let saved = save(&repository, create_input(), 100).expect("save command");

        assert_eq!(saved.name, "Docker logs");
        assert_eq!(saved.description, "Follow logs");
        assert_eq!(saved.tags, ["Docker", "logs"]);
        assert_eq!(saved.created_at, 100);
        assert_eq!(saved.updated_at, 100);
    }

    #[test]
    fn updates_preserve_server_owned_and_future_fields() {
        let database = Database::open_in_memory().expect("open database");
        let repository = CommandRepository::new(&database);
        let mut original = save(&repository, create_input(), 100).expect("create command");
        original.use_count = 7;
        original.last_used_at = Some(150);
        original.variables.push(CommandVariable {
            name: "container".to_string(),
            label: "Container".to_string(),
            value_type: "text".to_string(),
            default_value: None,
            options_json: None,
            is_required: true,
            position: 0,
        });
        repository.save(&original).expect("seed future fields");

        let mut input = create_input();
        input.id = Some(original.id.clone());
        input.name = "Updated".to_string();
        let updated = save(&repository, input, 200).expect("update command");

        assert_eq!(updated.id, original.id);
        assert_eq!(updated.created_at, 100);
        assert_eq!(updated.updated_at, 200);
        assert_eq!(updated.use_count, 7);
        assert_eq!(updated.last_used_at, Some(150));
        assert_eq!(updated.variables, original.variables);
    }

    #[test]
    fn saves_variables_and_confirmation_from_the_input() {
        let database = Database::open_in_memory().expect("open database");
        let repository = CommandRepository::new(&database);
        let mut input = create_input();
        input.command_text = "ssh {{user}}@{{host}} -p {{port}}".to_string();
        input.confirm_before_run = Some(true);
        input.variables = Some(vec![
            CommandVariable {
                label: "  Login  ".to_string(),
                default_value: Some("root".to_string()),
                // Options only belong to choices.
                options_json: Some("[\"a\"]".to_string()),
                ..variable(" user ", "text")
            },
            CommandVariable {
                default_value: Some("c".to_string()),
                options_json: Some("[\" a \", \"b\", \"a\", \"\"]".to_string()),
                ..variable("host", "choice")
            },
            CommandVariable {
                default_value: Some("hunter2".to_string()),
                ..variable("port", "secret")
            },
        ]);

        let saved = save(&repository, input, 100).expect("save command");
        assert!(saved.confirm_before_run);
        assert_eq!(
            saved.variables,
            vec![
                CommandVariable {
                    name: "user".to_string(),
                    label: "Login".to_string(),
                    default_value: Some("root".to_string()),
                    position: 0,
                    ..variable("user", "text")
                },
                CommandVariable {
                    label: "host".to_string(),
                    // A default outside the options is dropped.
                    default_value: None,
                    options_json: Some("[\"a\",\"b\"]".to_string()),
                    position: 1,
                    ..variable("host", "choice")
                },
                CommandVariable {
                    label: "port".to_string(),
                    default_value: None,
                    position: 2,
                    ..variable("port", "secret")
                },
            ]
        );
        assert_eq!(
            repository.get(&saved.id).expect("get command"),
            Some(saved.clone())
        );

        // An explicit empty list removes them; `false` clears the confirmation.
        let mut update = create_input();
        update.id = Some(saved.id);
        update.confirm_before_run = Some(false);
        update.variables = Some(Vec::new());
        let updated = save(&repository, update, 200).expect("update command");
        assert!(!updated.confirm_before_run);
        assert!(updated.variables.is_empty());
    }

    #[test]
    fn rejects_malformed_variables() {
        let database = Database::open_in_memory().expect("open database");
        let repository = CommandRepository::new(&database);
        let cases: [(Vec<CommandVariable>, &str); 5] = [
            (vec![variable("1st", "text")], "must start with a letter"),
            (vec![variable("a b", "text")], "must start with a letter"),
            (
                vec![variable("host", "text"), variable("host", "number")],
                "defined twice",
            ),
            (vec![variable("env", "choice")], "invalid options"),
            (
                vec![CommandVariable {
                    default_value: Some("many".to_string()),
                    ..variable("count", "number")
                }],
                "not a number",
            ),
        ];

        for (variables, expected) in cases {
            let mut input = create_input();
            input.variables = Some(variables);
            let error = save(&repository, input, 100).expect_err("malformed variables must fail");
            assert!(error.contains(expected), "{error}");
        }
        assert!(repository.list().expect("list commands").is_empty());
    }

    #[test]
    fn update_requires_an_existing_id() {
        let database = Database::open_in_memory().expect("open database");
        let repository = CommandRepository::new(&database);
        let mut input = create_input();
        input.id = Some("missing".to_string());

        assert_eq!(
            save(&repository, input, 100).expect_err("missing update must fail"),
            "Saved command not found"
        );
    }

    #[test]
    fn rejects_whitespace_only_command_text() {
        let database = Database::open_in_memory().expect("open database");
        let repository = CommandRepository::new(&database);
        let mut input = create_input();
        input.command_text = "  \n  ".to_string();

        assert!(save(&repository, input, 100)
            .expect_err("blank command must fail")
            .contains("Command text must contain"));
    }
}
