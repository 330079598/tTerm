//! Persists a live scrollback edit without replacing other settings with a startup snapshot.

use std::{fs, io::ErrorKind, path::Path};

use indexmap::IndexMap;
use serde_json::value::{RawValue, to_raw_value};

use super::AppConfig;
use super::atomic::atomic_write;
use super::paths::ensure_config_dir;
use super::scrollback::normalize_scrollback_config;

/// Updates the scrollback field in the shared file format, or creates a complete first config.
pub fn save_scrollback_lines(lines: u32) -> Result<(), String> {
    save_at(&ensure_config_dir()?.join("config.json"), lines)
}

fn save_at(path: &Path, lines: u32) -> Result<(), String> {
    let lines = normalize_scrollback_config(Some(f64::from(lines)));
    let previous = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let config = AppConfig {
                scrollback_lines: lines,
                ..AppConfig::default()
            };
            return super::compatible_file::save_at(path, &config);
        }
        Err(error) => {
            return Err(format!(
                "Cannot read settings at '{}': {error}",
                path.display()
            ));
        }
    };
    serde_json::from_str::<AppConfig>(&previous).map_err(|error| {
        format!(
            "Repair the settings at '{}' before saving: {error}",
            path.display()
        )
    })?;
    let mut fields: IndexMap<String, &RawValue> = serde_json::from_str(&previous)
        .map_err(|error| format!("Cannot read settings fields: {error}"))?;
    let value = to_raw_value(&lines).map_err(|error| error.to_string())?;
    fields.insert("scrollback_lines".into(), &value);
    let bytes = serde_json::to_vec_pretty(&fields).map_err(|error| error.to_string())?;
    atomic_write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tauri_and_native_edits_round_trip_without_resetting_unrelated_settings() {
        let path = std::env::temp_dir().join(format!("config-{}.json", uuid::Uuid::new_v4()));
        save_at(&path, 0).unwrap();
        let mut from_native = crate::config::load_config_at(&path).unwrap();
        assert_eq!(from_native.scrollback_lines, 0);
        from_native.font_size = 24;
        crate::config::compatible_file::save_at(&path, &from_native).unwrap();
        let extended =
            fs::read_to_string(&path)
                .unwrap()
                .replacen('{', r#"{"future":1.2300e+45,"#, 1);
        fs::write(&path, extended).unwrap();
        save_at(&path, 372).unwrap();
        let reloaded = crate::config::load_config_at(&path).unwrap();
        assert_eq!((reloaded.font_size, reloaded.scrollback_lines), (24, 372));
        assert!(fs::read_to_string(&path).unwrap().contains("1.2300e+45"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn an_unreadable_config_is_not_overwritten_with_defaults() {
        let path = std::env::temp_dir().join(format!("config-{}.json", uuid::Uuid::new_v4()));
        for damaged in ["{", r#"{"theme":false}"#] {
            fs::write(&path, damaged).unwrap();
            assert!(save_at(&path, 8).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), damaged);
        }
        fs::remove_file(path).unwrap();
    }
}
