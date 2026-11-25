//! Terminal page saves: dirty fields, disk readback and refusals.

use std::fs;
use std::path::Path;

use super::super::AppConfig;
use super::*;
use crate::config::scrollback::MAX_EXPLICIT_SCROLLBACK_LINES;

fn temp_config() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("config-{}.json", uuid::Uuid::new_v4()))
}

fn seed(path: &Path, config: &AppConfig) {
    super::super::compatible_file::save_at(path, config).unwrap();
}

#[test]
fn applies_only_the_changed_fields_to_the_latest_file() {
    let path = temp_config();
    seed(
        &path,
        &AppConfig {
            font_family: "Menlo".to_string(),
            font_size: 14,
            terminal_line_height: 1.2,
            terminal_letter_spacing: 1,
            scrollback_lines: 10_000,
            ..AppConfig::default()
        },
    );
    let snapshot = super::super::load_config_at(&path).unwrap();

    // Another client changes the theme, one page field and the scrollback
    // between the snapshot this window loaded and its save.
    let mut other_client = super::super::load_config_at(&path).unwrap();
    other_client.theme = "nord".to_string();
    other_client.terminal_letter_spacing = 4;
    other_client.scrollback_lines = 25_000;
    seed(&path, &other_client);
    let extended = fs::read_to_string(&path)
        .unwrap()
        .replacen('{', r#"{"future":1.2300e+45,"#, 1);
    fs::write(&path, extended).unwrap();

    let mut draft = TerminalPageSettings::from(&snapshot);
    draft.family = "Fira Code".to_string();
    draft.size = 16;
    draft.line_height = 1.75;

    let saved = save_page_at(&path, &snapshot, draft).unwrap();

    assert_eq!(
        saved,
        TerminalPageSettings {
            family: "Fira Code".to_string(),
            size: 16,
            line_height: 1.75,
            letter_spacing: 4,
            scrollback_lines: 25_000,
        },
        "the untouched page fields keep the values the other client wrote"
    );
    let reloaded = super::super::load_config_at(&path).unwrap();
    assert_eq!(reloaded.theme, "nord");
    assert_eq!(reloaded.font_family, "Fira Code");
    assert_eq!(reloaded.font_size, 16);
    assert_eq!(reloaded.terminal_line_height, 1.75);
    assert_eq!(reloaded.terminal_letter_spacing, 4);
    assert_eq!(reloaded.scrollback_lines, 25_000);
    assert!(fs::read_to_string(&path).unwrap().contains("1.2300e+45"));
    fs::remove_file(path).unwrap();
}

#[test]
fn reports_the_values_the_file_holds() {
    let path = temp_config();
    let previous = AppConfig::default();

    let saved = save_page_at(
        &path,
        &previous,
        TerminalPageSettings {
            family: "  \"JetBrains Mono\", monospace  ".to_string(),
            size: 21,
            line_height: 1.236,
            letter_spacing: -2,
            scrollback_lines: 0,
        },
    )
    .unwrap();

    assert_eq!(
        saved,
        TerminalPageSettings::from(&super::super::load_config_at(&path).unwrap())
    );
    assert_eq!(saved.family, "\"JetBrains Mono\", monospace");
    assert_eq!(saved.size, 21);
    assert_eq!(saved.line_height, 1.24);
    assert_eq!(saved.letter_spacing, -2);
    assert_eq!(saved.scrollback_lines, 0);
    fs::remove_file(path).unwrap();
}

#[test]
fn an_unchanged_page_writes_nothing() {
    let path = temp_config();
    let previous = AppConfig::default();

    let saved = save_page_at(&path, &previous, TerminalPageSettings::from(&previous)).unwrap();

    assert_eq!(saved, TerminalPageSettings::from(&previous));
    assert!(
        !path.exists(),
        "an unchanged page must not create or rewrite a config file"
    );
}

#[test]
fn refuses_an_invalid_page_without_touching_the_file() {
    let path = temp_config();
    let previous = AppConfig::default();
    seed(&path, &previous);
    let before = fs::read(&path).unwrap();
    let base = TerminalPageSettings::from(&previous);

    for settings in [
        TerminalPageSettings {
            family: String::new(),
            ..base.clone()
        },
        TerminalPageSettings {
            family: " \t ".to_string(),
            ..base.clone()
        },
        TerminalPageSettings {
            family: "Menlo\nmonospace".to_string(),
            ..base.clone()
        },
        TerminalPageSettings {
            size: 5,
            ..base.clone()
        },
        TerminalPageSettings {
            size: 73,
            ..base.clone()
        },
        TerminalPageSettings {
            line_height: 0.99,
            ..base.clone()
        },
        TerminalPageSettings {
            line_height: 2.01,
            ..base.clone()
        },
        TerminalPageSettings {
            line_height: f64::NAN,
            ..base.clone()
        },
        TerminalPageSettings {
            letter_spacing: -6,
            ..base.clone()
        },
        TerminalPageSettings {
            letter_spacing: 11,
            ..base.clone()
        },
        TerminalPageSettings {
            scrollback_lines: MAX_EXPLICIT_SCROLLBACK_LINES + 1,
            ..base.clone()
        },
    ] {
        assert!(
            save_page_at(&path, &previous, settings.clone()).is_err(),
            "{settings:?} must be refused"
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    let missing = temp_config();
    assert!(
        save_page_at(
            &missing,
            &previous,
            TerminalPageSettings { size: 100, ..base }
        )
        .is_err()
    );
    assert!(!missing.exists());
    fs::remove_file(path).unwrap();
}

#[test]
fn refuses_to_merge_into_an_unreadable_config() {
    let path = temp_config();
    let previous = AppConfig::default();
    let draft = TerminalPageSettings {
        size: 20,
        ..TerminalPageSettings::from(&previous)
    };

    for damaged in ["{", r#"{"theme":false}"#] {
        fs::write(&path, damaged).unwrap();
        assert!(save_page_at(&path, &previous, draft.clone()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), damaged);
    }
    fs::remove_file(path).unwrap();
}
