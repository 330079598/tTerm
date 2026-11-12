use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandVariable {
    pub name: String,
    pub label: String,
    pub value_type: String,
    pub default_value: Option<String>,
    pub options_json: Option<String>,
    pub is_required: bool,
    pub position: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedCommand {
    pub id: String,
    pub name: String,
    pub command_text: String,
    pub description: String,
    pub scope_type: String,
    pub scope_id: Option<String>,
    pub shell_type: String,
    pub platform: String,
    pub is_favorite: bool,
    pub confirm_before_run: bool,
    pub sort_order: i64,
    pub use_count: i64,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub tags: Vec<String>,
    pub variables: Vec<CommandVariable>,
}

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
