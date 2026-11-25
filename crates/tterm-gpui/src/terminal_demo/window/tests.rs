use tterm_core::config::AppConfig;
use tterm_core::events::ssh::SshConnectionProgressPayload;

use super::events::connection_state;
use super::{DEFAULT_TITLE, TitleUpdate, classify_title_update, startup_config};
use crate::terminal_demo::binding::scoped::ConnectionState;

#[test]
fn connection_progress_maps_to_the_scoped_connection_state() {
    assert_eq!(
        connection_state(&SshConnectionProgressPayload::new("ready", "ready")),
        ConnectionState::Connected
    );
    assert_eq!(
        connection_state(&SshConnectionProgressPayload::new("retrying", "retrying")),
        ConnectionState::Reconnecting
    );
    assert_eq!(
        connection_state(&SshConnectionProgressPayload::new(
            "retry_exhausted",
            "exhausted"
        )),
        ConnectionState::Error
    );
    assert_eq!(
        connection_state(&SshConnectionProgressPayload::new(
            "host_key_checking",
            "checking"
        )),
        ConnectionState::Connecting
    );
}

#[test]
fn default_title_marker_resolves_through_the_locale() {
    assert_eq!(
        classify_title_update(Some(DEFAULT_TITLE.to_string())),
        Some(TitleUpdate::Default)
    );
    assert_eq!(classify_title_update(None), None);
}

#[test]
fn a_program_title_is_preserved_verbatim() {
    assert_eq!(
        classify_title_update(Some("cargo test".to_string())),
        Some(TitleUpdate::Program("cargo test".to_string()))
    );
}

#[test]
fn startup_config_normalizes_before_views_read_it() {
    let loaded = AppConfig {
        ui_style: "floating".to_string(),
        ui_scale_percent: 85,
        close_behavior: "close".to_string(),
        ..AppConfig::default()
    };

    let config = startup_config(Ok(loaded));

    assert_eq!(config.ui_style, "classic");
    assert_eq!(config.ui_scale_percent, 90);
    assert_eq!(config.close_behavior, "ask");
}

#[test]
fn startup_config_uses_normalized_defaults_when_reading_fails() {
    let config = startup_config(Err("damaged config".to_string()));

    assert_eq!(config.ui_style, "islands");
    assert_eq!(config.secret_storage_mode, "system");
}
