//! Persists only the saved-connection order in the shared config file.
//!
//! Choosing an order is a narrow edit: it starts from the latest
//! `config.json` while holding the shared edit lock, changes `profile_sort`,
//! and writes the result through the compatible-file writer. A setting another
//! client changed just before it, and a key this version does not know,
//! therefore survive the save.

use std::path::Path;

use super::paths::ensure_config_dir;

/// The values `profile_sort` accepts.
const PROFILE_SORTS: [&str; 2] = ["default", "recent"];

/// Saves the order the saved-connection picker lists profiles in.
pub fn save_profile_sort(sort: &str) -> Result<(), String> {
    save_at(&ensure_config_dir()?.join("config.json"), sort)
}

pub(super) fn save_at(path: &Path, sort: &str) -> Result<(), String> {
    if !PROFILE_SORTS.contains(&sort) {
        return Err(format!(
            "The profile order '{sort}' is not 'default' or 'recent'."
        ));
    }
    let _edit = super::edit::lock()?;
    let mut config = super::load_config_at(path)?;
    if config.profile_sort == sort {
        return Ok(());
    }
    config.profile_sort = sort.to_string();
    super::compatible_file::save_at(path, &config)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use indexmap::IndexMap;
    use serde_json::value::RawValue;

    use super::*;
    use crate::config::AppConfig;

    fn temp_path() -> PathBuf {
        std::env::temp_dir().join(format!("config-{}.json", uuid::Uuid::new_v4()))
    }

    #[test]
    fn changing_the_order_keeps_unknown_fields_and_a_recent_scrollback_edit() {
        let path = temp_path();
        let config = AppConfig {
            font_size: 23,
            ..AppConfig::default()
        };
        crate::config::compatible_file::save_at(&path, &config).unwrap();
        let raw = r#"[123456789012345678901234567890,1.2300e+05,null]"#;
        let extended =
            fs::read_to_string(&path)
                .unwrap()
                .replacen('{', &format!(r#"{{"future":{raw},"#), 1);
        fs::write(&path, extended).unwrap();
        // Another narrow setting writer saves after the extend above, just
        // before the order changes.
        crate::config::scrollback_file::save_at(&path, 372).unwrap();

        save_at(&path, "recent").unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let fields: IndexMap<String, &RawValue> = serde_json::from_str(&content).unwrap();
        assert_eq!(fields["future"].get(), raw);
        let reloaded = crate::config::load_config_at(&path).unwrap();
        assert_eq!(reloaded.profile_sort, "recent");
        assert_eq!(reloaded.font_size, 23);
        assert_eq!(reloaded.scrollback_lines, 372);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_an_unknown_order_without_touching_the_file() {
        let path = temp_path();
        crate::config::compatible_file::save_at(&path, &AppConfig::default()).unwrap();
        let before = fs::read_to_string(&path).unwrap();
        assert!(save_at(&path, "sideways").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        fs::remove_file(path).unwrap();
    }
}
