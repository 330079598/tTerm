//! Reading and writing an agent's JSON settings file without disturbing it:
//! keys keep their order, the previous file is kept as `<name>.tterm-bak`,
//! and a symlinked file (dotfile managers) is written through, not replaced.

use std::fs;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// A JSON value whose objects keep their key order (serde_json's sort them).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub(super) enum Json {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Json>),
    Object(Object),
}

pub(super) type Object = IndexMap<String, Json>;

impl Json {
    pub(super) fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(value) => Some(value),
            _ => None,
        }
    }
}

/// The file's top-level object; an empty one when the file does not exist or
/// holds only whitespace.
pub(super) fn read_object(path: &Path) -> Result<Object, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Object::new()),
        Err(error) => return Err(format!("Failed to read {}: {error}", path.display())),
    };
    parse_object(&text).map_err(|error| format!("{}: {error}", path.display()))
}

pub(super) fn parse_object(text: &str) -> Result<Object, String> {
    if text.trim().is_empty() {
        return Ok(Object::new());
    }
    match serde_json::from_str::<Json>(text) {
        Ok(Json::Object(object)) => Ok(object),
        Ok(_) => Err("expected a JSON object".to_string()),
        Err(error) => Err(format!("not valid JSON ({error})")),
    }
}

/// Two-space indentation and a final newline, as the agents write it.
pub(super) fn to_text(object: &Object) -> String {
    let mut text = serde_json::to_string_pretty(object).unwrap_or_else(|_| "{}".to_string());
    text.push('\n');
    text
}

pub(super) fn write_object(path: &Path, object: &Object) -> Result<(), String> {
    let target = resolve_symlink(path);
    let text = to_text(object);
    match fs::read_to_string(&target) {
        Ok(previous) if previous == text => return Ok(()),
        Ok(previous) => {
            let backup = backup_path(&target);
            fs::write(&backup, previous)
                .map_err(|error| format!("Failed to back up to {}: {error}", backup.display()))?;
        }
        Err(_) => {}
    }
    crate::config::atomic_write(&target, text)
}

fn resolve_symlink(path: &Path) -> PathBuf {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
        }
        _ => path.to_path_buf(),
    }
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tterm-bak");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tterm-agent-json-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn keeps_key_order_and_values() {
        let text = "{\n  \"zeta\": 1,\n  \"alpha\": {\"b\": [true, null, 1.5], \"a\": \"x\"}\n}\n";
        let object = parse_object(text).unwrap();
        assert_eq!(
            to_text(&object),
            "{\n  \"zeta\": 1,\n  \"alpha\": {\n    \"b\": [\n      true,\n      null,\n      1.5\n    ],\n    \"a\": \"x\"\n  }\n}\n"
        );
    }

    #[test]
    fn treats_a_missing_or_blank_file_as_empty() {
        let dir = temp_dir("missing");
        assert!(read_object(&dir.join("settings.json")).unwrap().is_empty());
        assert!(parse_object(" \n").unwrap().is_empty());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn refuses_what_is_not_a_json_object() {
        assert!(parse_object("[1]").is_err());
        assert!(parse_object("{ // comment\n}").is_err());
    }

    #[test]
    fn backs_up_the_previous_file_and_skips_unchanged_writes() {
        let dir = temp_dir("backup");
        let path = dir.join("settings.json");
        fs::write(&path, "{\"a\": 1}").unwrap();

        let mut object = read_object(&path).unwrap();
        object.insert("b".to_string(), Json::Bool(true));
        write_object(&path, &object).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("settings.json.tterm-bak")).unwrap(),
            "{\"a\": 1}"
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "{\n  \"a\": 1,\n  \"b\": true\n}\n"
        );

        fs::remove_file(dir.join("settings.json.tterm-bak")).unwrap();
        write_object(&path, &object).unwrap();
        assert!(!dir.join("settings.json.tterm-bak").exists());
        fs::remove_dir_all(dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn writes_through_a_symlink() {
        let dir = temp_dir("symlink");
        let real = dir.join("dotfiles-settings.json");
        let link = dir.join("settings.json");
        fs::write(&real, "{}").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let mut object = Object::new();
        object.insert("a".to_string(), Json::Bool(true));
        write_object(&link, &object).unwrap();
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), "{\n  \"a\": true\n}\n");
        fs::remove_dir_all(dir).ok();
    }
}
