use super::{CommandLibraryState, CommandRepository, SavedCommand};
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashSet;
use tauri::State;

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
}

fn default_scope_type() -> String {
    "global".to_string()
}

#[tauri::command]
pub fn list_saved_commands(
    state: State<'_, CommandLibraryState>,
) -> Result<Vec<SavedCommand>, String> {
    CommandRepository::new(state.database()?).list()
}

#[tauri::command]
pub fn list_command_tags(state: State<'_, CommandLibraryState>) -> Result<Vec<String>, String> { CommandRepository::new(state.database()?).list_tags() }

#[tauri::command]
pub fn create_command_tag(state: State<'_, CommandLibraryState>, tag: String) -> Result<String, String> { CommandRepository::new(state.database()?).create_tag(&tag) }

#[tauri::command]
pub fn rename_command_tag(state: State<'_, CommandLibraryState>, old_tag: String, new_tag: String) -> Result<Vec<String>, String> { CommandRepository::new(state.database()?).rename_tag(&old_tag, &new_tag) }

#[tauri::command]
pub fn delete_command_tag(state: State<'_, CommandLibraryState>, tag: String) -> Result<Vec<String>, String> { CommandRepository::new(state.database()?).delete_tag(&tag) }

#[tauri::command]
pub fn save_saved_command(
    state: State<'_, CommandLibraryState>,
    input: SaveCommandInput,
) -> Result<SavedCommand, String> {
    save(
        &CommandRepository::new(state.database()?),
        input,
        Utc::now().timestamp_millis(),
    )
}

#[tauri::command]
pub fn delete_saved_command(
    state: State<'_, CommandLibraryState>,
    id: String,
) -> Result<bool, String> {
    CommandRepository::new(state.database()?).delete(id.trim())
}

#[tauri::command]
pub fn set_saved_command_favorite(
    state: State<'_, CommandLibraryState>,
    id: String,
    favorite: bool,
) -> Result<SavedCommand, String> {
    let repository = CommandRepository::new(state.database()?);
    let mut command = repository
        .get(id.trim())?
        .ok_or_else(|| "Saved command not found".to_string())?;
    command.is_favorite = favorite;
    command.updated_at = Utc::now().timestamp_millis();
    repository.save(&command)?;
    Ok(command)
}

#[tauri::command]
pub fn record_saved_command_use(
    state: State<'_, CommandLibraryState>,
    id: String,
) -> Result<(), String> {
    let updated = CommandRepository::new(state.database()?)
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
        confirm_before_run: existing
            .as_ref()
            .is_some_and(|command| command.confirm_before_run),
        sort_order: existing.as_ref().map_or(0, |command| command.sort_order),
        use_count: existing.as_ref().map_or(0, |command| command.use_count),
        last_used_at: existing.as_ref().and_then(|command| command.last_used_at),
        created_at: existing.as_ref().map_or(now, |command| command.created_at),
        updated_at: now,
        tags,
        variables: existing
            .map(|command| command.variables)
            .unwrap_or_default(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_library::{CommandDatabase, CommandVariable};

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
        }
    }

    #[test]
    fn creates_and_normalizes_user_input() {
        let database = CommandDatabase::open_in_memory().expect("open database");
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
        let database = CommandDatabase::open_in_memory().expect("open database");
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
    fn update_requires_an_existing_id() {
        let database = CommandDatabase::open_in_memory().expect("open database");
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
        let database = CommandDatabase::open_in_memory().expect("open database");
        let repository = CommandRepository::new(&database);
        let mut input = create_input();
        input.command_text = "  \n  ".to_string();

        assert!(save(&repository, input, 100)
            .expect_err("blank command must fail")
            .contains("Command text must contain"));
    }
}
