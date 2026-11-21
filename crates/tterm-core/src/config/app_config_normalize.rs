//! UI-side settings normalization without changing the config file reader.

use super::AppConfig;
use super::limits::TERMINAL_LINE_HEIGHT_RANGE;
use crate::workspace::tab_settings::TabSettings;
use tterm_terminal::theme::js::js_round;

mod fields;
#[cfg(test)]
mod oracle;
mod scalars;

/// Normalizes a partial UI configuration without reading or writing the settings file.
/// Unknown top-level fields are retained separately by the local save path.
pub fn normalize_config(input: &serde_json::Value) -> serde_json::Result<AppConfig> {
    let mut config = AppConfig::default();
    fields::passthrough(&mut config, input);
    scalars::flags(&mut config, input);
    scalars::choices(&mut config, input);
    scalars::numbers(&mut config, input);
    fields::collections(&mut config, input)?;
    apply_tab_settings(
        &mut config,
        crate::workspace::tab_settings::normalize_tab_settings(input),
    )?;
    Ok(config)
}

/// Normalizes the config a strict reader accepted, as the Tauri client does.
///
/// The Tauri client receives an [`AppConfig`] from `load_config` and runs the
/// frontend's `normalizeConfig` before any view reads it. Native startup uses
/// this entry so the same file produces the same choices in both clients. The
/// strict reader still decides whether the file is readable; a damaged file is
/// not repaired here.
pub fn normalize_loaded_config(config: &AppConfig) -> serde_json::Result<AppConfig> {
    normalize_config(&serde_json::to_value(config)?)
}

/// Rounds UI scale to the nearest ten percent within the supported range.
pub fn normalize_ui_scale_percent(value: Option<f64>) -> u16 {
    let Some(value) = value.filter(|value| value.is_finite()) else {
        return 100;
    };
    (js_round(value / 10.0) * 10.0).clamp(80.0, 200.0) as u16
}

/// Rounds the line height to two decimals as in the settings form.
pub fn normalize_terminal_line_height(value: Option<f64>) -> f64 {
    let Some(value) = value.filter(|value| value.is_finite()) else {
        return 1.0;
    };
    (js_round(value * 100.0) / 100.0).clamp(
        *TERMINAL_LINE_HEIGHT_RANGE.start(),
        *TERMINAL_LINE_HEIGHT_RANGE.end(),
    )
}

/// Copies normalized tab settings into their persisted representation, leaving
/// unrelated settings and the destination on conversion failure untouched.
pub fn apply_tab_settings(config: &mut AppConfig, settings: TabSettings) -> serde_json::Result<()> {
    let icons = serde_json::from_value(serde_json::to_value(settings.tab_icons)?)?;
    let indicator = serde_json::from_value(serde_json::to_value(settings.tab_agent_indicator)?)?;
    let panes = serde_json::from_value(serde_json::to_value(settings.panes)?)?;
    config.tab_icons = icons;
    config.tab_agent_indicator = indicator;
    config.panes = panes;
    config.tab_width_mode = settings.tab_width_mode;
    config.tab_standard_width = settings.tab_standard_width;
    config.tab_hover_tooltip = settings.tab_hover_tooltip;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tab_settings::normalize_tab_settings;
    use serde_json::json;

    #[test]
    fn numeric_settings_follow_js_rounding_and_limits() {
        for value in [None, Some(f64::INFINITY), Some(f64::NAN)] {
            assert_eq!(normalize_ui_scale_percent(value), 100);
            assert_eq!(normalize_terminal_line_height(value), 1.0);
        }
        assert_eq!(normalize_ui_scale_percent(Some(85.0)), 90);
        assert_eq!(normalize_ui_scale_percent(Some(550.0)), 200);
        assert_eq!(normalize_terminal_line_height(Some(1.236)), 1.24);
        assert_eq!(normalize_terminal_line_height(Some(f64::MAX)), 2.0);
    }

    #[test]
    fn tab_conversion_preserves_persisted_shapes() {
        let root = json!({"type":"split","orientation":"horizontal","children":[{"type":"pane"},{"type":"pane"}],"sizes":[0.30000000000000004,0.7]});
        let settings = normalize_tab_settings(
            &json!({"tab_standard_width":165,"tab_icons":{"enabled":false},"tab_hover_tooltip":true,"panes":{"templates":[{"id":"layout","name":"Layout","root":root}]}}),
        );
        let indicator = serde_json::to_string(&settings.tab_agent_indicator).unwrap();
        let mut config = AppConfig::default();
        apply_tab_settings(&mut config, settings).unwrap();
        assert_eq!(config.tab_standard_width, 165);
        assert!(!config.tab_icons.enabled && config.tab_hover_tooltip);
        assert_eq!(
            serde_json::to_string(&config.tab_agent_indicator).unwrap(),
            indicator
        );
        assert_eq!(config.panes.templates[0].root.to_string(), root.to_string());
    }

    #[test]
    fn a_strictly_loaded_config_is_normalized_like_the_tauri_frontend() {
        let loaded: AppConfig = serde_json::from_value(json!({
            "theme": "default",
            "ui_style": "floating",
            "close_behavior": "close",
            "sftp_start_directory": "root",
            "secret_storage_mode": "hybrid",
            "hidden_renderer_release_secs": 7,
            "favorite_themes": ["nord", "nord", ""],
            "sftp_external_editor": "  code  ",
            "ui_scale_percent": 85
        }))
        .unwrap();

        let config = normalize_loaded_config(&loaded).unwrap();

        assert_eq!(config.ui_scale_percent, 90);
        assert_eq!(config.ui_style, "classic");
        assert_eq!(config.close_behavior, "ask");
        assert_eq!(config.sftp_start_directory, "last");
        assert_eq!(config.secret_storage_mode, "system");
        assert_eq!(
            config.hidden_renderer_release_secs,
            crate::config::limits::DEFAULT_HIDDEN_RENDERER_RELEASE_SECS
        );
        assert_eq!(config.favorite_themes, vec!["nord".to_string()]);
        assert_eq!(config.sftp_external_editor, "code");
    }

    #[test]
    fn a_first_launch_keeps_the_native_layout_and_storage_defaults() {
        let config = normalize_loaded_config(&AppConfig::default()).unwrap();

        assert_eq!(config.theme, "default");
        assert_eq!(config.ui_style, "islands");
        assert_eq!(config.secret_storage_mode, "system");
    }
}
