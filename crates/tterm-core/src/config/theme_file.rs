//! Persists a theme choice without replacing other settings with a startup
//! snapshot.
//!
//! The native client repaints from the accepted save and also loads these
//! fields on the next launch. The edit writes the fields that differ from the
//! caller's snapshot, in one atomic replacement, so an unrelated setting, a key
//! this version does not know, or a custom theme id another client saved
//! survives. A changed field must name one of the six built-in themes or a
//! theme in the bundled library.

use std::path::Path;

use super::AppConfig;
use super::paths::ensure_config_dir;
use crate::theme::catalog::load_theme_catalog;
use crate::theme::definitions;

/// The theme fields the settings pages share with `config.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeSettings {
    /// The theme shown when the client does not follow the system.
    pub theme: String,
    /// Whether the light and dark slots follow the system appearance.
    pub follow_system: bool,
    /// The theme for a light system appearance.
    pub theme_light: String,
    /// The theme for a dark system appearance.
    pub theme_dark: String,
}

impl From<&AppConfig> for ThemeSettings {
    fn from(config: &AppConfig) -> Self {
        Self {
            theme: config.theme.clone(),
            follow_system: config.theme_follow_system,
            theme_light: config.theme_light.clone(),
            theme_dark: config.theme_dark.clone(),
        }
    }
}

/// Replaces the changed theme fields in the shared config file.
///
/// `previous` is the snapshot the caller last loaded or saved. Only a field
/// that differs from it is written. A changed id that is not one of the six
/// built-in themes or a theme in the bundled library, or an unreadable config,
/// leaves the file as it was. The result carries every theme value the next
/// launch will load.
pub fn save_theme_settings(
    previous: &AppConfig,
    settings: ThemeSettings,
) -> Result<ThemeSettings, String> {
    save_at(
        &ensure_config_dir()?.join("config.json"),
        previous,
        settings,
    )
}

pub(super) fn save_at(
    path: &Path,
    previous: &AppConfig,
    settings: ThemeSettings,
) -> Result<ThemeSettings, String> {
    let previous = ThemeSettings::from(previous);
    let _edit = super::edit::lock()?;
    let mut config = super::load_config_at(path)?;
    if settings != previous {
        apply_theme_settings(&mut config, &previous, &settings)?;
        super::compatible_file::save_at(path, &config)?;
        config = super::load_config_at(path)?;
    }
    Ok(ThemeSettings::from(&config))
}

/// Applies only the changed theme fields to `config`.
///
/// Every changed id is checked before anything is written, so an invalid
/// draft cannot leave a half-applied edit. The returned snapshot describes
/// the config after the edit. Test writers use the same rules as the file
/// path.
pub fn apply_theme_settings(
    config: &mut AppConfig,
    previous: &ThemeSettings,
    settings: &ThemeSettings,
) -> Result<ThemeSettings, String> {
    let theme = changed_id("theme", &previous.theme, &settings.theme)?;
    let theme_light = changed_id(
        "light appearance",
        &previous.theme_light,
        &settings.theme_light,
    )?;
    let theme_dark = changed_id(
        "dark appearance",
        &previous.theme_dark,
        &settings.theme_dark,
    )?;
    if settings.follow_system != previous.follow_system {
        config.theme_follow_system = settings.follow_system;
    }
    if let Some(id) = theme {
        config.theme = id.to_string();
    }
    if let Some(id) = theme_light {
        config.theme_light = id.to_string();
    }
    if let Some(id) = theme_dark {
        config.theme_dark = id.to_string();
    }
    Ok(ThemeSettings::from(&*config))
}

/// Returns the new id when the field changed, refusing a value neither the
/// built-in themes nor the bundled library can render.
fn changed_id<'a>(field: &str, previous: &str, next: &'a str) -> Result<Option<&'a str>, String> {
    if previous == next {
        return Ok(None);
    }
    if known_theme_id(next) {
        return Ok(Some(next));
    }
    Err(format!(
        "The {field} theme '{next}' is not one of the six built-in themes: Default, Light, Ocean, Forest, Sunset or Ubuntu, nor a theme in the bundled library."
    ))
}

/// Whether the native client can render an id as a preset or library theme.
fn known_theme_id(theme_id: &str) -> bool {
    definitions::get_preset_theme(theme_id).is_some()
        || load_theme_catalog().is_ok_and(|themes| themes.iter().any(|theme| theme.id == theme_id))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    fn seeded(config: &AppConfig) -> (PathBuf, String) {
        let path = std::env::temp_dir().join(format!("config-{}.json", uuid::Uuid::new_v4()));
        let content = serde_json::to_string_pretty(config).expect("config serializes");
        fs::write(&path, &content).expect("seed config");
        (path, content)
    }

    #[test]
    fn a_theme_edit_keeps_unrelated_and_unknown_settings() {
        let config = AppConfig {
            scrollback_lines: 12_345,
            font_family: "Fixture Mono".to_string(),
            theme: "default".to_string(),
            ..AppConfig::default()
        };
        let (path, content) = seeded(&config);
        let extended = content.replacen(
            '{',
            r#"{"future_setting":{"precision":123456789012345678901234567890},"#,
            1,
        );
        fs::write(&path, &extended).expect("extend config");

        let saved = save_at(
            &path,
            &config,
            ThemeSettings {
                theme: "ocean".to_string(),
                follow_system: true,
                theme_light: "light".to_string(),
                theme_dark: "forest".to_string(),
            },
        )
        .expect("theme save");

        assert_eq!(saved.theme, "ocean");
        assert!(saved.follow_system);
        assert_eq!(saved.theme_light, "light");
        assert_eq!(saved.theme_dark, "forest");
        let reloaded = crate::config::load_config_at(&path).expect("reload config");
        assert_eq!(reloaded.theme, "ocean");
        assert!(reloaded.theme_follow_system);
        assert_eq!(reloaded.theme_light, "light");
        assert_eq!(reloaded.theme_dark, "forest");
        assert_eq!(reloaded.scrollback_lines, 12_345);
        assert_eq!(reloaded.font_family, "Fixture Mono");
        let saved_json = fs::read_to_string(&path).expect("read config");
        let fields: indexmap::IndexMap<String, &serde_json::value::RawValue> =
            serde_json::from_str(&saved_json).expect("parse config fields");
        assert_eq!(
            fields["future_setting"].get(),
            r#"{"precision":123456789012345678901234567890}"#
        );
        fs::remove_file(path).expect("remove config");
    }

    #[test]
    fn an_unchanged_field_keeps_a_theme_id_the_native_client_cannot_show() {
        let config = AppConfig {
            theme: "custom-from-tauri".to_string(),
            theme_light: "catalog:fixture".to_string(),
            ..AppConfig::default()
        };
        let (path, _) = seeded(&config);

        let saved = save_at(
            &path,
            &config,
            ThemeSettings {
                theme: "custom-from-tauri".to_string(),
                follow_system: true,
                theme_light: "catalog:fixture".to_string(),
                theme_dark: "sunset".to_string(),
            },
        )
        .expect("theme save");

        assert_eq!(saved.theme, "custom-from-tauri");
        assert_eq!(saved.theme_light, "catalog:fixture");
        assert_eq!(saved.theme_dark, "sunset");
        let reloaded = crate::config::load_config_at(&path).expect("reload config");
        assert_eq!(reloaded.theme, "custom-from-tauri");
        assert_eq!(reloaded.theme_light, "catalog:fixture");
        fs::remove_file(path).expect("remove config");
    }

    #[test]
    fn a_bundled_library_theme_can_be_staged_in_every_slot() {
        let themes = load_theme_catalog().expect("the bundled catalog");
        let light = themes
            .iter()
            .find(|theme| !theme.is_dark())
            .expect("a light library theme");
        let dark = themes
            .iter()
            .find(|theme| theme.is_dark())
            .expect("a dark library theme");
        let config = AppConfig {
            scrollback_lines: 54_321,
            ..AppConfig::default()
        };
        let (path, _) = seeded(&config);

        let saved = save_at(
            &path,
            &config,
            ThemeSettings {
                theme: light.id.clone(),
                follow_system: true,
                theme_light: light.id.clone(),
                theme_dark: dark.id.clone(),
            },
        )
        .expect("library theme save");

        assert_eq!(saved.theme, light.id);
        assert!(saved.follow_system);
        assert_eq!(saved.theme_light, light.id);
        assert_eq!(saved.theme_dark, dark.id);
        let reloaded = crate::config::load_config_at(&path).expect("reload config");
        assert_eq!(reloaded.theme, light.id);
        assert!(reloaded.theme_follow_system);
        assert_eq!(reloaded.theme_light, light.id);
        assert_eq!(reloaded.theme_dark, dark.id);
        assert_eq!(
            reloaded.scrollback_lines, 54_321,
            "a library theme edit keeps the rest of the config"
        );
        fs::remove_file(path).expect("remove config");
    }

    #[test]
    fn an_unknown_library_id_leaves_the_file_untouched() {
        let (path, content) = seeded(&AppConfig::default());
        let error = save_at(
            &path,
            &AppConfig::default(),
            ThemeSettings {
                theme: "catalog:No Such Bundled Theme".to_string(),
                follow_system: false,
                theme_light: "light".to_string(),
                theme_dark: "default".to_string(),
            },
        )
        .expect_err("an unknown library id is refused");

        assert!(error.contains("catalog:No Such Bundled Theme"), "{error}");
        assert_eq!(fs::read_to_string(&path).expect("read config"), content);
        fs::remove_file(path).expect("remove config");
    }

    #[test]
    fn an_unknown_changed_id_leaves_the_file_untouched() {
        let (path, content) = seeded(&AppConfig::default());
        let error = save_at(
            &path,
            &AppConfig::default(),
            ThemeSettings {
                theme: "custom-from-tauri".to_string(),
                follow_system: false,
                theme_light: "light".to_string(),
                theme_dark: "default".to_string(),
            },
        )
        .expect_err("an unknown theme id is refused");

        assert!(error.contains("custom-from-tauri"), "{error}");
        assert_eq!(fs::read_to_string(&path).expect("read config"), content);
        fs::remove_file(path).expect("remove config");
    }
}
