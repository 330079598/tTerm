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
            cursor_style: "bar".to_string(),
            terminal_padding_left_px: 7,
            terminal_padding_right_px: 8,
            terminal_padding_bottom_px: 9,
            confirm_multiline_paste: false,
            copy_on_select: true,
            right_click_paste: false,
            osc52_clipboard_write: true,
            mac_option_is_meta: false,
            ..AppConfig::default()
        },
    );
    let snapshot = super::super::load_config_at(&path).unwrap();

    // Another client changes the theme, several page fields, the scrollback
    // and the clipboard preferences between the snapshot this window loaded
    // and its save.
    let mut other_client = super::super::load_config_at(&path).unwrap();
    other_client.theme = "nord".to_string();
    other_client.terminal_letter_spacing = 4;
    other_client.scrollback_lines = 25_000;
    other_client.cursor_style = "underline".to_string();
    other_client.terminal_padding_left_px = 20;
    other_client.terminal_padding_right_px = 21;
    other_client.terminal_padding_bottom_px = 22;
    other_client.confirm_multiline_paste = true;
    other_client.copy_on_select = false;
    other_client.right_click_paste = true;
    other_client.osc52_clipboard_write = false;
    other_client.mac_option_is_meta = true;
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
            cursor_style: "underline".to_string(),
            padding_left: 20,
            padding_right: 21,
            padding_bottom: 22,
            confirm_multiline_paste: true,
            copy_on_select: false,
            right_click_paste: true,
            osc52_clipboard_write: false,
            mac_option_is_meta: true,
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
    assert_eq!(reloaded.cursor_style, "underline");
    assert_eq!(
        (
            reloaded.terminal_padding_left_px,
            reloaded.terminal_padding_right_px,
            reloaded.terminal_padding_bottom_px,
        ),
        (20, 21, 22)
    );
    assert_eq!(
        (
            reloaded.confirm_multiline_paste,
            reloaded.copy_on_select,
            reloaded.right_click_paste,
            reloaded.osc52_clipboard_write,
            reloaded.mac_option_is_meta,
        ),
        (true, false, true, false, true)
    );
    assert!(fs::read_to_string(&path).unwrap().contains("1.2300e+45"));
    fs::remove_file(path).unwrap();
}

#[test]
fn a_cursor_and_padding_edit_keeps_the_latest_font_and_scrollback() {
    let path = temp_config();
    seed(
        &path,
        &AppConfig {
            font_family: "Menlo".to_string(),
            font_size: 14,
            terminal_line_height: 1.2,
            terminal_letter_spacing: 1,
            scrollback_lines: 10_000,
            confirm_multiline_paste: true,
            copy_on_select: true,
            right_click_paste: false,
            ..AppConfig::default()
        },
    );
    let snapshot = super::super::load_config_at(&path).unwrap();

    // Another client changes everything except the cursor and padding between
    // this window's snapshot and its save.
    let mut other_client = super::super::load_config_at(&path).unwrap();
    other_client.font_family = "Fira Code".to_string();
    other_client.font_size = 18;
    other_client.terminal_line_height = 1.6;
    other_client.terminal_letter_spacing = 3;
    other_client.scrollback_lines = 25_000;
    other_client.confirm_multiline_paste = false;
    other_client.copy_on_select = false;
    other_client.right_click_paste = true;
    seed(&path, &other_client);

    let mut draft = TerminalPageSettings::from(&snapshot);
    draft.cursor_style = "bar".to_string();
    draft.padding_left = 12;
    draft.padding_right = 14;
    draft.padding_bottom = 8;

    let saved = save_page_at(&path, &snapshot, draft).unwrap();

    assert_eq!(
        (
            saved.family.as_str(),
            saved.size,
            saved.line_height,
            saved.letter_spacing,
            saved.scrollback_lines,
            saved.cursor_style.as_str(),
            saved.padding_left,
            saved.padding_right,
            saved.padding_bottom,
            saved.confirm_multiline_paste,
            saved.copy_on_select,
            saved.right_click_paste,
        ),
        (
            "Fira Code",
            18,
            1.6,
            3,
            25_000,
            "bar",
            12,
            14,
            8,
            false,
            false,
            true
        ),
        "an old field the other client changed must survive a page edit"
    );
    let reloaded = super::super::load_config_at(&path).unwrap();
    assert_eq!(
        (
            reloaded.font_family.as_str(),
            reloaded.font_size,
            reloaded.terminal_line_height,
            reloaded.terminal_letter_spacing,
            reloaded.scrollback_lines,
            reloaded.cursor_style.as_str(),
            reloaded.terminal_padding_left_px,
            reloaded.terminal_padding_right_px,
            reloaded.terminal_padding_bottom_px,
            reloaded.confirm_multiline_paste,
            reloaded.copy_on_select,
            reloaded.right_click_paste,
        ),
        (
            "Fira Code",
            18,
            1.6,
            3,
            25_000,
            "bar",
            12,
            14,
            8,
            false,
            false,
            true
        )
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn a_clipboard_edit_writes_only_its_field_and_keeps_an_external_page_edit() {
    let path = temp_config();
    seed(
        &path,
        &AppConfig {
            font_family: "Menlo".to_string(),
            font_size: 14,
            confirm_multiline_paste: true,
            copy_on_select: false,
            right_click_paste: false,
            ..AppConfig::default()
        },
    );
    let snapshot = super::super::load_config_at(&path).unwrap();

    // Another client changes the font and the preference this window keeps,
    // between this window's snapshot and its clipboard save.
    let mut other_client = super::super::load_config_at(&path).unwrap();
    other_client.font_family = "Fira Code".to_string();
    other_client.font_size = 18;
    other_client.confirm_multiline_paste = false;
    other_client.right_click_paste = true;
    seed(&path, &other_client);

    let mut draft = TerminalPageSettings::from(&snapshot);
    draft.copy_on_select = true;

    let saved = save_page_at(&path, &snapshot, draft).unwrap();

    assert_eq!(
        (
            saved.family.as_str(),
            saved.size,
            saved.confirm_multiline_paste,
            saved.copy_on_select,
            saved.right_click_paste,
        ),
        ("Fira Code", 18, false, true, true),
        "only the changed preference is written; the latest file wins the rest"
    );
    let reloaded = super::super::load_config_at(&path).unwrap();
    assert_eq!(
        (
            reloaded.font_family.as_str(),
            reloaded.font_size,
            reloaded.confirm_multiline_paste,
            reloaded.copy_on_select,
            reloaded.right_click_paste,
        ),
        ("Fira Code", 18, false, true, true)
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn an_osc52_and_option_edit_writes_only_its_fields_and_keeps_an_external_page_edit() {
    let path = temp_config();
    seed(
        &path,
        &AppConfig {
            font_family: "Menlo".to_string(),
            font_size: 14,
            confirm_multiline_paste: true,
            copy_on_select: false,
            osc52_clipboard_write: true,
            mac_option_is_meta: false,
            ..AppConfig::default()
        },
    );
    let snapshot = super::super::load_config_at(&path).unwrap();

    // Another client changes settings this window keeps, including the
    // clipboard preference beside the two this window saves.
    let mut other_client = super::super::load_config_at(&path).unwrap();
    other_client.font_family = "Fira Code".to_string();
    other_client.font_size = 18;
    other_client.confirm_multiline_paste = false;
    other_client.copy_on_select = true;
    seed(&path, &other_client);

    let mut draft = TerminalPageSettings::from(&snapshot);
    draft.osc52_clipboard_write = false;
    draft.mac_option_is_meta = true;

    let saved = save_page_at(&path, &snapshot, draft).unwrap();

    assert_eq!(
        (
            saved.family.as_str(),
            saved.size,
            saved.confirm_multiline_paste,
            saved.copy_on_select,
            saved.osc52_clipboard_write,
            saved.mac_option_is_meta,
        ),
        ("Fira Code", 18, false, true, false, true),
        "only the changed preferences are written; the latest file wins the rest"
    );
    let reloaded = super::super::load_config_at(&path).unwrap();
    assert_eq!(
        (
            reloaded.font_family.as_str(),
            reloaded.font_size,
            reloaded.confirm_multiline_paste,
            reloaded.copy_on_select,
            reloaded.osc52_clipboard_write,
            reloaded.mac_option_is_meta,
        ),
        ("Fira Code", 18, false, true, false, true)
    );
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
            cursor_style: "underline".to_string(),
            padding_left: 80,
            padding_right: 0,
            padding_bottom: 80,
            confirm_multiline_paste: false,
            copy_on_select: true,
            right_click_paste: true,
            osc52_clipboard_write: false,
            mac_option_is_meta: true,
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
    assert_eq!(saved.cursor_style, "underline");
    assert_eq!(
        (
            saved.padding_left,
            saved.padding_right,
            saved.padding_bottom
        ),
        (80, 0, 80)
    );
    assert!(!saved.confirm_multiline_paste);
    assert!(saved.copy_on_select);
    assert!(saved.right_click_paste);
    assert!(!saved.osc52_clipboard_write);
    assert!(saved.mac_option_is_meta);
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
        TerminalPageSettings {
            cursor_style: "beam".to_string(),
            ..base.clone()
        },
        TerminalPageSettings {
            cursor_style: String::new(),
            ..base.clone()
        },
        TerminalPageSettings {
            padding_left: 81,
            ..base.clone()
        },
        TerminalPageSettings {
            padding_right: 81,
            ..base.clone()
        },
        TerminalPageSettings {
            padding_bottom: 81,
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

#[test]
fn refuses_a_clipboard_edit_to_an_unreadable_config() {
    let path = temp_config();
    let previous = AppConfig::default();
    let mut draft = TerminalPageSettings::from(&previous);
    draft.confirm_multiline_paste = !draft.confirm_multiline_paste;
    draft.copy_on_select = !draft.copy_on_select;
    draft.right_click_paste = !draft.right_click_paste;

    for damaged in ["{", r#"{"confirm_multiline_paste":false}"#] {
        fs::write(&path, damaged).unwrap();
        assert!(save_page_at(&path, &previous, draft.clone()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), damaged);
    }
    fs::remove_file(path).unwrap();
}

#[test]
fn refuses_an_osc52_and_option_edit_to_an_unreadable_config() {
    let path = temp_config();
    let previous = AppConfig::default();
    let mut draft = TerminalPageSettings::from(&previous);
    draft.osc52_clipboard_write = !draft.osc52_clipboard_write;
    draft.mac_option_is_meta = !draft.mac_option_is_meta;

    for damaged in ["{", r#"{"osc52_clipboard_write":false}"#] {
        fs::write(&path, damaged).unwrap();
        assert!(save_page_at(&path, &previous, draft.clone()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), damaged);
    }
    fs::remove_file(path).unwrap();
}
