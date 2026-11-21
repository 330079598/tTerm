//! Preserves future top-level settings locally without adding them to IPC or sync.

use super::AppConfig;
use super::atomic::atomic_write;
use indexmap::IndexMap;
use serde_json::value::RawValue;
use std::{fs, io::ErrorKind, path::Path};

type Fields<'a> = IndexMap<String, &'a RawValue>;

/// Writes known settings and carries over unknown JSON values from the destination.
pub(super) fn save_at(path: &Path, config: &AppConfig) -> Result<(), String> {
    let known = serde_json::to_string_pretty(config)
        .map_err(|error| format!("Failed to serialize config: {error}"))?;
    let previous = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => return atomic_write(path, known),
        Err(error) => {
            return Err(format!(
                "Cannot preserve settings in '{}': {error}",
                path.display()
            ));
        }
    };
    let mut merged: Fields = serde_json::from_str(&known)
        .map_err(|error| format!("Failed to serialize config fields: {error}"))?;
    let old: Fields = serde_json::from_str(&previous).map_err(|error| {
        format!(
            "Cannot preserve settings in '{}': {error}. Repair the file before saving.",
            path.display()
        )
    })?;
    for (key, raw) in old {
        if key != "local_shell_integration" {
            merged.entry(key).or_insert(raw);
        }
    }
    let content = serde_json::to_vec_pretty(&merged)
        .map_err(|error| format!("Failed to serialize merged config: {error}"))?;
    atomic_write(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_known_settings_without_rounding_future_values_or_reviving_legacy_keys() {
        let path = std::env::temp_dir().join(format!("config-{}.json", uuid::Uuid::new_v4()));
        save_at(&path, &AppConfig::default()).unwrap();
        let seed = fs::read_to_string(&path).unwrap();
        let raw = r#"{"nested":[123456789012345678901234567890,1.2300e+05,null]}"#;
        let extended = seed.replacen(
            '{',
            &format!(r#"{{"future":{raw},"local_shell_integration":true,"#),
            1,
        );
        fs::write(&path, extended).unwrap();
        let mut config = super::super::load_config_at(&path).unwrap();
        config.font_size = 23;
        save_at(&path, &config).unwrap();
        let result = fs::read_to_string(&path).unwrap();
        let fields: Fields = serde_json::from_str(&result).unwrap();
        assert_eq!(fields["future"].get(), raw);
        assert!(!fields.contains_key("local_shell_integration"));
        let reloaded = super::super::load_config_at(&path).unwrap();
        assert_eq!(reloaded.font_size, 23);
        assert!(reloaded.local_unix_shell_integration);
        assert!(
            serde_json::to_value(reloaded)
                .unwrap()
                .get("future")
                .is_none()
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_unreadable_json_objects() {
        let path = std::env::temp_dir().join(format!("config-{}.json", uuid::Uuid::new_v4()));
        for damaged in ["{", "[]", "null"] {
            fs::write(&path, damaged).unwrap();
            assert!(save_at(&path, &AppConfig::default()).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), damaged);
        }
        fs::remove_file(path).unwrap();
    }
}
