use super::AppConfig;

#[test]
fn legacy_config_shows_its_one_theme() {
    let config: AppConfig = serde_json::from_str(r#"{"theme":"catalog:Nord"}"#).unwrap();

    assert_eq!(config.theme, "catalog:Nord");
    assert!(!config.theme_follow_system);
    assert_eq!(config.theme_light, "light");
    assert_eq!(config.theme_dark, "default");
    assert!(config.favorite_themes.is_empty());
}

#[test]
fn legacy_config_uses_adaptive_tab_width_defaults() {
    let config: AppConfig = serde_json::from_str(r#"{"theme":"default"}"#).unwrap();

    assert_eq!(config.tab_width_mode, "adaptive");
    assert_eq!(config.tab_standard_width, 120);
    assert!(!config.tab_hover_tooltip);
    assert_eq!(config.ui_scale_percent, 100);
    assert!(!config.terminal_log_enabled);
    assert_eq!(config.terminal_log_format, "both");
    assert_eq!(config.terminal_log_max_file_size_mb, 50);
    assert!(config.terminal_log_plain_timestamps);
    assert_eq!(
        config.monitor_visible_metrics,
        ["cpu", "memory", "network", "ip", "latency", "disk"]
    );
    assert_eq!(config.sftp_transfer_parallelism, 4);
    assert!(config.reconnect_enabled);
    assert_eq!(config.reconnect_max_attempts, 5);
}

#[test]
fn explicit_sftp_transfer_parallelism_round_trips() {
    let config: AppConfig =
        serde_json::from_str(r#"{"theme":"default","sftp_transfer_parallelism":6}"#).unwrap();

    assert_eq!(config.sftp_transfer_parallelism, 6);
}

#[test]
fn sftp_bandwidth_limits_default_to_unlimited_and_round_trip() {
    let legacy: AppConfig = serde_json::from_str(r#"{"theme":"default"}"#).unwrap();
    assert_eq!(legacy.sftp_upload_limit_kib, 0);
    assert_eq!(legacy.sftp_download_limit_kib, 0);

    let config: AppConfig = serde_json::from_str(
        r#"{"theme":"default","sftp_upload_limit_kib":512,"sftp_download_limit_kib":2048}"#,
    )
    .unwrap();
    assert_eq!(config.sftp_upload_limit_kib, 512);
    assert_eq!(config.sftp_download_limit_kib, 2048);
}

#[test]
fn reconnect_enabled_round_trips() {
    let disabled: AppConfig =
        serde_json::from_str(r#"{"theme":"default","reconnect_enabled":false}"#).unwrap();
    assert!(!disabled.reconnect_enabled);

    let enabled: AppConfig =
        serde_json::from_str(r#"{"theme":"default","reconnect_enabled":true}"#).unwrap();
    assert!(enabled.reconnect_enabled);

    let serialized = serde_json::to_string(&disabled).unwrap();
    let reparsed: AppConfig = serde_json::from_str(&serialized).unwrap();
    assert!(!reparsed.reconnect_enabled);
}

#[test]
fn reconnect_max_attempts_round_trips() {
    let custom: AppConfig =
        serde_json::from_str(r#"{"theme":"default","reconnect_max_attempts":12}"#).unwrap();
    assert_eq!(custom.reconnect_max_attempts, 12);

    let legacy: AppConfig = serde_json::from_str(r#"{"theme":"default"}"#).unwrap();
    assert_eq!(legacy.reconnect_max_attempts, 5);
}

#[test]
fn legacy_config_gets_default_keymap() {
    let config: AppConfig = serde_json::from_str(r#"{"theme":"default"}"#).unwrap();

    assert!(config.keymap.bindings.is_empty());
}

#[test]
fn keymap_config_round_trips() {
    let config: AppConfig = serde_json::from_str(
        r#"{"theme":"default","keymap":{"bindings":{"terminal.find":["mod+j"],"terminal.clear":null}}}"#,
    )
    .unwrap();

    assert_eq!(
        config.keymap.bindings.get("terminal.find"),
        Some(&Some(vec!["mod+j".to_string()]))
    );
    assert_eq!(config.keymap.bindings.get("terminal.clear"), Some(&None));

    let serialized = serde_json::to_string(&config).unwrap();
    let reparsed: AppConfig = serde_json::from_str(&serialized).unwrap();
    assert_eq!(
        reparsed.keymap.bindings.get("terminal.find"),
        config.keymap.bindings.get("terminal.find")
    );
}

#[test]
fn keymap_config_ignores_removed_preset_field() {
    let config: AppConfig =
        serde_json::from_str(r#"{"theme":"default","keymap":{"preset":"vscode","bindings":{}}}"#)
            .unwrap();

    assert!(config.keymap.bindings.is_empty());
    assert!(!serde_json::to_string(&config).unwrap().contains("preset"));
}

#[test]
fn keymap_config_accepts_legacy_single_chord_values() {
    let config: AppConfig = serde_json::from_str(
        r#"{"theme":"default","keymap":{"bindings":{"terminal.find":"mod+j","editor.save":null}}}"#,
    )
    .unwrap();

    assert_eq!(
        config.keymap.bindings.get("terminal.find"),
        Some(&Some(vec!["mod+j".to_string()]))
    );
    assert_eq!(config.keymap.bindings.get("editor.save"), Some(&None));
}

#[test]
fn explicit_tab_width_config_is_deserialized() {
    let config: AppConfig = serde_json::from_str(
        r#"{"theme":"default","tab_width_mode":"standard","tab_standard_width":180}"#,
    )
    .unwrap();

    assert_eq!(config.tab_width_mode, "standard");
    assert_eq!(config.tab_standard_width, 180);
}

#[test]
fn invalid_tab_width_config_is_normalized() {
    let config: AppConfig = serde_json::from_str(
        r#"{"theme":"default","tab_width_mode":"wide","tab_standard_width":79.6}"#,
    )
    .unwrap();

    assert_eq!(config.tab_width_mode, "adaptive");
    assert_eq!(config.tab_standard_width, 80);
}

#[test]
fn terminal_line_height_and_letter_spacing_are_clamped() {
    let config: AppConfig = serde_json::from_str(
        r#"{"theme":"default","terminal_line_height":1.234,"terminal_letter_spacing":-1.6}"#,
    )
    .unwrap();
    assert_eq!(config.terminal_line_height, 1.23);
    assert_eq!(config.terminal_letter_spacing, -2);

    let config: AppConfig = serde_json::from_str(
        r#"{"theme":"default","terminal_line_height":0.5,"terminal_letter_spacing":40}"#,
    )
    .unwrap();
    assert_eq!(config.terminal_line_height, 1.0);
    assert_eq!(config.terminal_letter_spacing, 10);
}

#[test]
fn ui_scale_config_is_rounded_and_clamped() {
    let rounded: AppConfig =
        serde_json::from_str(r#"{"theme":"default","ui_scale_percent":146}"#).unwrap();
    let minimum: AppConfig =
        serde_json::from_str(r#"{"theme":"default","ui_scale_percent":40}"#).unwrap();
    let maximum: AppConfig =
        serde_json::from_str(r#"{"theme":"default","ui_scale_percent":260}"#).unwrap();

    assert_eq!(rounded.ui_scale_percent, 150);
    assert_eq!(minimum.ui_scale_percent, 80);
    assert_eq!(maximum.ui_scale_percent, 200);
}

#[test]
fn window_blur_config_is_clamped() {
    let parse = |field: &str, value: &str| -> AppConfig {
        serde_json::from_str(&format!(r#"{{"theme":"default","{field}":{value}}}"#)).unwrap()
    };

    assert_eq!(
        parse("window_opacity_percent", "62.4").window_opacity_percent,
        62
    );
    assert_eq!(
        parse("window_opacity_percent", "10").window_opacity_percent,
        30
    );
    assert_eq!(
        parse("window_opacity_percent", "100").window_opacity_percent,
        95
    );
    assert_eq!(parse("window_blur_radius", "0").window_blur_radius, 1);
    assert_eq!(parse("window_blur_radius", "200").window_blur_radius, 60);
    let missing: AppConfig = serde_json::from_str(r#"{"theme":"default"}"#).unwrap();
    assert!(!missing.window_blur);
    assert_eq!(missing.window_blur_radius, 20);
    assert_eq!(missing.window_opacity_percent, 70);
    assert_eq!(missing.window_blur_material, "acrylic");
}

#[test]
fn tab_icons_default_to_shown_with_built_in_looks() {
    let config: AppConfig = serde_json::from_str(r#"{"theme":"default"}"#).unwrap();

    assert!(config.tab_icons.enabled);
    assert!(config.tab_icons.shells.is_empty());
}

#[test]
fn tab_icons_keep_each_shells_choice() {
    let config: AppConfig = serde_json::from_str(
        r##"{"theme":"default","tab_icons":{"enabled":false,"shells":{"pwsh":{"color":"#ff0000"}}}}"##,
    )
    .unwrap();

    assert!(!config.tab_icons.enabled);
    let pwsh = &config.tab_icons.shells["pwsh"];
    assert_eq!(pwsh.icon, None);
    assert_eq!(pwsh.color.as_deref(), Some("#ff0000"));
    let saved = serde_json::to_string(&config.tab_icons).unwrap();
    assert!(saved.contains(r##""pwsh":{"color":"#ff0000"}"##));
}

#[test]
fn tab_agent_indicator_defaults_to_the_disc_and_pause_bars() {
    let config: AppConfig = serde_json::from_str(r#"{"theme":"default"}"#).unwrap();

    assert_eq!(config.tab_agent_indicator.working, "disc");
    assert_eq!(config.tab_agent_indicator.waiting, "bars");
    assert!(config.tab_agent_indicator.working_color.is_empty());
}

#[test]
fn tab_agent_indicator_keeps_a_partial_choice() {
    let config: AppConfig = serde_json::from_str(
        r##"{"theme":"default","tab_agent_indicator":{"working":"braille","waiting_color":"#ff0000"}}"##,
    )
    .unwrap();

    assert_eq!(config.tab_agent_indicator.working, "braille");
    assert_eq!(config.tab_agent_indicator.waiting, "bars");
    assert_eq!(config.tab_agent_indicator.waiting_color, "#ff0000");
}
