use super::{CommandRepository, CommandVariable, SavedCommand};
use crate::core::blocking::run_blocking;
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
pub async fn list_saved_commands() -> Result<Vec<SavedCommand>, String> {
    run_blocking(|| CommandRepository::new(crate::db::get()?).list()).await
}

#[tauri::command]
pub async fn list_command_tags() -> Result<Vec<String>, String> {
    run_blocking(|| CommandRepository::new(crate::db::get()?).list_tags()).await
}

#[tauri::command]
pub async fn create_command_tag(tag: String) -> Result<String, String> {
    run_blocking(move || CommandRepository::new(crate::db::get()?).create_tag(&tag)).await
}

#[tauri::command]
pub async fn rename_command_tag(old_tag: String, new_tag: String) -> Result<Vec<String>, String> {
    run_blocking(move || CommandRepository::new(crate::db::get()?).rename_tag(&old_tag, &new_tag))
        .await
}

#[tauri::command]
pub async fn delete_command_tag(tag: String) -> Result<Vec<String>, String> {
    run_blocking(move || CommandRepository::new(crate::db::get()?).delete_tag(&tag)).await
}

#[tauri::command]
pub async fn save_saved_command(input: SaveCommandInput) -> Result<SavedCommand, String> {
    run_blocking(move || {
        save(
            &CommandRepository::new(crate::db::get()?),
            input,
            Utc::now().timestamp_millis(),
        )
    })
    .await
}

#[tauri::command]
pub async fn delete_saved_command(id: String) -> Result<bool, String> {
    run_blocking(move || CommandRepository::new(crate::db::get()?).delete(id.trim())).await
}

#[tauri::command]
pub async fn set_saved_command_favorite(
    id: String,
    favorite: bool,
) -> Result<SavedCommand, String> {
    run_blocking(move || {
        let repository = CommandRepository::new(crate::db::get()?);
        let mut command = repository
            .get(id.trim())?
            .ok_or_else(|| "Saved command not found".to_string())?;
        command.is_favorite = favorite;
        command.updated_at = Utc::now().timestamp_millis();
        repository.save(&command)?;
        Ok(command)
    })
    .await
}

#[tauri::command]
pub async fn record_saved_command_use(id: String) -> Result<(), String> {
    run_blocking(move || {
        let updated = CommandRepository::new(crate::db::get()?)
            .record_use(id.trim(), Utc::now().timestamp_millis())?;
        if updated {
            Ok(())
        } else {
            Err("Saved command not found".to_string())
        }
    })
    .await
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
mod tests;
