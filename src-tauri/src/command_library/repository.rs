use super::{CommandDatabase, CommandVariable, SavedCommand};
use rusqlite::{params, OptionalExtension, Row, Transaction};

pub struct CommandRepository<'a> {
    database: &'a CommandDatabase,
}

impl<'a> CommandRepository<'a> {
    pub fn new(database: &'a CommandDatabase) -> Self {
        Self { database }
    }

    pub fn list(&self) -> Result<Vec<SavedCommand>, String> {
        let connection = self
            .database
            .connection
            .lock()
            .map_err(|_| "Command database lock was poisoned".to_string())?;
        let mut statement = connection
            .prepare(
                "SELECT id, name, command_text, description, scope_type, scope_id, shell_type, \
                        platform, is_favorite, confirm_before_run, sort_order, use_count, \
                        last_used_at, created_at, updated_at \
                 FROM saved_commands \
                 ORDER BY is_favorite DESC, sort_order ASC, updated_at DESC",
            )
            .map_err(database_error("prepare command list"))?;
        let rows = statement
            .query_map([], map_command)
            .map_err(database_error("query commands"))?;
        let mut commands = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error("read commands"))?;
        drop(statement);

        for command in &mut commands {
            load_relations(&connection, command)?;
        }
        Ok(commands)
    }

    pub fn get(&self, id: &str) -> Result<Option<SavedCommand>, String> {
        let connection = self
            .database
            .connection
            .lock()
            .map_err(|_| "Command database lock was poisoned".to_string())?;
        let mut command = connection
            .query_row(
                "SELECT id, name, command_text, description, scope_type, scope_id, shell_type, \
                        platform, is_favorite, confirm_before_run, sort_order, use_count, \
                        last_used_at, created_at, updated_at \
                 FROM saved_commands WHERE id = ?1",
                params![id],
                map_command,
            )
            .optional()
            .map_err(database_error("get command"))?;
        if let Some(command) = command.as_mut() {
            load_relations(&connection, command)?;
        }
        Ok(command)
    }

    pub fn save(&self, command: &SavedCommand) -> Result<(), String> {
        validate(command)?;
        let mut connection = self
            .database
            .connection
            .lock()
            .map_err(|_| "Command database lock was poisoned".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(database_error("start command save transaction"))?;

        transaction
            .execute(
                "INSERT INTO saved_commands (
                    id, name, command_text, description, scope_type, scope_id, shell_type, \
                    platform, is_favorite, confirm_before_run, sort_order, use_count, \
                    last_used_at, created_at, updated_at
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
                 ) ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name, command_text = excluded.command_text,
                    description = excluded.description, scope_type = excluded.scope_type,
                    scope_id = excluded.scope_id, shell_type = excluded.shell_type,
                    platform = excluded.platform, is_favorite = excluded.is_favorite,
                    confirm_before_run = excluded.confirm_before_run,
                    sort_order = excluded.sort_order, use_count = excluded.use_count,
                    last_used_at = excluded.last_used_at, updated_at = excluded.updated_at",
                params![
                    command.id,
                    command.name,
                    command.command_text,
                    command.description,
                    command.scope_type,
                    command.scope_id,
                    command.shell_type,
                    command.platform,
                    command.is_favorite,
                    command.confirm_before_run,
                    command.sort_order,
                    command.use_count,
                    command.last_used_at,
                    command.created_at,
                    command.updated_at,
                ],
            )
            .map_err(database_error("save command"))?;

        replace_relations(&transaction, command)?;
        refresh_search_index(&transaction, command)?;
        transaction
            .commit()
            .map_err(database_error("commit command save"))
    }

    pub fn delete(&self, id: &str) -> Result<bool, String> {
        let mut connection = self
            .database
            .connection
            .lock()
            .map_err(|_| "Command database lock was poisoned".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(database_error("start command delete transaction"))?;
        transaction
            .execute(
                "DELETE FROM saved_commands_fts WHERE command_id = ?1",
                params![id],
            )
            .map_err(database_error("delete command search index"))?;
        let affected = transaction
            .execute("DELETE FROM saved_commands WHERE id = ?1", params![id])
            .map_err(database_error("delete command"))?;
        transaction
            .commit()
            .map_err(database_error("commit command delete"))?;
        Ok(affected > 0)
    }

    pub fn record_use(&self, id: &str, used_at: i64) -> Result<bool, String> {
        let connection = self
            .database
            .connection
            .lock()
            .map_err(|_| "Command database lock was poisoned".to_string())?;
        let affected = connection
            .execute(
                "UPDATE saved_commands \
                 SET use_count = use_count + 1, last_used_at = ?2 \
                 WHERE id = ?1",
                params![id, used_at],
            )
            .map_err(database_error("record command use"))?;
        Ok(affected > 0)
    }
}

fn map_command(row: &Row<'_>) -> rusqlite::Result<SavedCommand> {
    Ok(SavedCommand {
        id: row.get(0)?,
        name: row.get(1)?,
        command_text: row.get(2)?,
        description: row.get(3)?,
        scope_type: row.get(4)?,
        scope_id: row.get(5)?,
        shell_type: row.get(6)?,
        platform: row.get(7)?,
        is_favorite: row.get(8)?,
        confirm_before_run: row.get(9)?,
        sort_order: row.get(10)?,
        use_count: row.get(11)?,
        last_used_at: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        tags: Vec::new(),
        variables: Vec::new(),
    })
}

fn load_relations(
    connection: &rusqlite::Connection,
    command: &mut SavedCommand,
) -> Result<(), String> {
    let mut tag_statement = connection
        .prepare("SELECT tag FROM command_tags WHERE command_id = ?1 ORDER BY normalized_tag")
        .map_err(database_error("prepare command tags"))?;
    command.tags = tag_statement
        .query_map(params![command.id], |row| row.get(0))
        .map_err(database_error("query command tags"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error("read command tags"))?;

    let mut variable_statement = connection
        .prepare(
            "SELECT name, label, value_type, default_value, options_json, is_required, position \
             FROM command_variables WHERE command_id = ?1 ORDER BY position, name",
        )
        .map_err(database_error("prepare command variables"))?;
    command.variables = variable_statement
        .query_map(params![command.id], |row| {
            Ok(CommandVariable {
                name: row.get(0)?,
                label: row.get(1)?,
                value_type: row.get(2)?,
                default_value: row.get(3)?,
                options_json: row.get(4)?,
                is_required: row.get(5)?,
                position: row.get(6)?,
            })
        })
        .map_err(database_error("query command variables"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error("read command variables"))?;
    Ok(())
}

fn replace_relations(transaction: &Transaction<'_>, command: &SavedCommand) -> Result<(), String> {
    transaction
        .execute(
            "DELETE FROM command_tags WHERE command_id = ?1",
            params![command.id],
        )
        .map_err(database_error("replace command tags"))?;
    for tag in &command.tags {
        let trimmed = tag.trim();
        transaction
            .execute(
                "INSERT OR IGNORE INTO command_tags (command_id, tag, normalized_tag) \
                 VALUES (?1, ?2, ?3)",
                params![command.id, trimmed, trimmed.to_lowercase()],
            )
            .map_err(database_error("save command tag"))?;
    }

    transaction
        .execute(
            "DELETE FROM command_variables WHERE command_id = ?1",
            params![command.id],
        )
        .map_err(database_error("replace command variables"))?;
    for variable in &command.variables {
        transaction
            .execute(
                "INSERT INTO command_variables (\
                    command_id, name, label, value_type, default_value, options_json, \
                    is_required, position\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    command.id,
                    variable.name,
                    variable.label,
                    variable.value_type,
                    variable.default_value,
                    variable.options_json,
                    variable.is_required,
                    variable.position,
                ],
            )
            .map_err(database_error("save command variable"))?;
    }
    Ok(())
}

fn refresh_search_index(
    transaction: &Transaction<'_>,
    command: &SavedCommand,
) -> Result<(), String> {
    transaction
        .execute(
            "DELETE FROM saved_commands_fts WHERE command_id = ?1",
            params![command.id],
        )
        .map_err(database_error("replace command search index"))?;
    transaction
        .execute(
            "INSERT INTO saved_commands_fts (command_id, name, command_text, description, tags) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                command.id,
                command.name,
                command.command_text,
                command.description,
                command.tags.join(" "),
            ],
        )
        .map_err(database_error("save command search index"))?;
    Ok(())
}

fn validate(command: &SavedCommand) -> Result<(), String> {
    if command.id.trim().is_empty() {
        return Err("Command id is required".to_string());
    }
    if command.name.trim().is_empty() || command.name.chars().count() > 120 {
        return Err("Command name must contain between 1 and 120 characters".to_string());
    }
    if command.command_text.trim().is_empty() || command.command_text.chars().count() > 65_536 {
        return Err("Command text must contain between 1 and 65536 characters".to_string());
    }
    if command.scope_type != "global" && command.scope_type != "profile" {
        return Err("Command scope type must be global or profile".to_string());
    }
    if (command.scope_type == "global" && command.scope_id.is_some())
        || (command.scope_type == "profile"
            && command
                .scope_id
                .as_deref()
                .is_none_or(|scope_id| scope_id.trim().is_empty()))
    {
        return Err("Command scope id does not match its scope type".to_string());
    }
    if !matches!(
        command.platform.as_str(),
        "any" | "windows" | "linux" | "macos"
    ) {
        return Err("Unsupported command platform".to_string());
    }
    if command.use_count < 0 {
        return Err("Command use count cannot be negative".to_string());
    }
    for tag in &command.tags {
        let length = tag.trim().chars().count();
        if length == 0 || length > 64 {
            return Err("Command tags must contain between 1 and 64 characters".to_string());
        }
    }
    for variable in &command.variables {
        if variable.name.trim().is_empty() || variable.name.chars().count() > 64 {
            return Err(
                "Command variable names must contain between 1 and 64 characters".to_string(),
            );
        }
        if variable.label.trim().is_empty() || variable.label.chars().count() > 120 {
            return Err(
                "Command variable labels must contain between 1 and 120 characters".to_string(),
            );
        }
        if !matches!(
            variable.value_type.as_str(),
            "text" | "number" | "choice" | "secret"
        ) {
            return Err("Unsupported command variable type".to_string());
        }
        if variable.value_type == "secret" && variable.default_value.is_some() {
            return Err(
                "Secret command variables cannot have persisted default values".to_string(),
            );
        }
    }
    Ok(())
}

fn database_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> String {
    move |error| format!("Failed to {context}: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_DATABASE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn sample_command() -> SavedCommand {
        SavedCommand {
            id: "command-1".to_string(),
            name: "Docker logs".to_string(),
            command_text: "docker logs -f {{container}}".to_string(),
            description: "Follow container logs".to_string(),
            scope_type: "global".to_string(),
            scope_id: None,
            shell_type: "any".to_string(),
            platform: "any".to_string(),
            is_favorite: true,
            confirm_before_run: false,
            sort_order: 10,
            use_count: 0,
            last_used_at: None,
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_000_000,
            tags: vec!["Docker".to_string(), "logs".to_string()],
            variables: vec![CommandVariable {
                name: "container".to_string(),
                label: "Container".to_string(),
                value_type: "text".to_string(),
                default_value: None,
                options_json: None,
                is_required: true,
                position: 0,
            }],
        }
    }

    #[test]
    fn saves_reads_and_updates_a_command_with_relations() {
        let database = CommandDatabase::open_in_memory().expect("open database");
        let repository = CommandRepository::new(&database);
        let mut command = sample_command();

        repository.save(&command).expect("save command");
        assert_eq!(
            repository.get(&command.id).expect("get command"),
            Some(command.clone())
        );

        command.name = "Container logs".to_string();
        command.tags = vec!["containers".to_string()];
        command.variables.clear();
        command.updated_at += 1;
        repository.save(&command).expect("update command");

        let saved = repository
            .get(&command.id)
            .expect("get updated command")
            .expect("command exists");
        assert_eq!(saved, command);
        assert_eq!(repository.list().expect("list commands"), vec![command]);
    }

    #[test]
    fn delete_cascades_relations_and_search_index() {
        let database = CommandDatabase::open_in_memory().expect("open database");
        let repository = CommandRepository::new(&database);
        let command = sample_command();
        repository.save(&command).expect("save command");

        assert!(repository.delete(&command.id).expect("delete command"));
        assert!(!repository
            .delete(&command.id)
            .expect("delete missing command"));
        assert_eq!(
            repository.get(&command.id).expect("get deleted command"),
            None
        );

        let connection = database.connection.lock().expect("lock database");
        for table in ["command_tags", "command_variables", "saved_commands_fts"] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("count related rows");
            assert_eq!(count, 0, "{table} should be empty");
        }
    }

    #[test]
    fn rejects_secret_defaults_before_writing() {
        let database = CommandDatabase::open_in_memory().expect("open database");
        let repository = CommandRepository::new(&database);
        let mut command = sample_command();
        command.variables[0].value_type = "secret".to_string();
        command.variables[0].default_value = Some("do-not-save".to_string());

        let error = repository
            .save(&command)
            .expect_err("secret default must fail");
        assert!(error.contains("cannot have persisted default values"));
        assert!(repository.list().expect("list commands").is_empty());
    }

    #[test]
    fn commands_persist_when_the_database_is_reopened() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let sequence = TEST_DATABASE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "tterm-command-library-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).expect("create test directory");
        let path = directory.join("tterm.db");
        let command = sample_command();

        {
            let database = CommandDatabase::open(&path).expect("open file database");
            CommandRepository::new(&database)
                .save(&command)
                .expect("save command");
        }

        {
            let database = CommandDatabase::open(&path).expect("reopen file database");
            let saved = CommandRepository::new(&database)
                .get(&command.id)
                .expect("read command");
            assert_eq!(saved, Some(command));
        }

        std::fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn records_command_use_without_changing_content() {
        let database = CommandDatabase::open_in_memory().expect("open database");
        let repository = CommandRepository::new(&database);
        let command = sample_command();
        repository.save(&command).expect("save command");

        assert!(repository
            .record_use(&command.id, 1_800_000_000_000)
            .expect("record use"));

        let used = repository
            .get(&command.id)
            .expect("get command")
            .expect("command exists");
        assert_eq!(used.use_count, 1);
        assert_eq!(used.last_used_at, Some(1_800_000_000_000));
        assert_eq!(used.name, command.name);
        assert!(!repository
            .record_use("missing", 1_800_000_000_001)
            .expect("record missing use"));
    }
}
