//! Persists Terminal font and page edits without replacing other settings with a
//! startup snapshot.
//!
//! Each edit starts from the latest `config.json` rather than the in-memory
//! snapshot, so a setting another client changed in the meantime, or a key
//! this version does not know, survives the save. The page save writes only
//! the fields that differ from the caller's snapshot, in one atomic
//! replacement. The values are read back from the file, so a caller publishes
//! exactly what the next launch loads.

use std::path::Path;

use super::AppConfig;
use super::app_config_normalize::normalize_terminal_line_height;
use super::limits::{
    TERMINAL_FONT_SIZE_RANGE, TERMINAL_LETTER_SPACING_RANGE, TERMINAL_LINE_HEIGHT_RANGE,
};
use super::paths::ensure_config_dir;
use super::scrollback::{MAX_EXPLICIT_SCROLLBACK_LINES, normalize_scrollback_config};
use tterm_terminal::js_text::js_trim;

const CURSOR_STYLES: [&str; 3] = ["block", "bar", "underline"];
const MAX_PAGE_PADDING: u16 = 80;

/// The Terminal font settings after a save, already read back from the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedTerminalFont {
    /// The CSS font-family chain the file now holds.
    pub family: String,
    /// The font size in logical pixels the file now holds.
    pub size: u16,
}

/// The complete Terminal page draft, saved as one configuration edit.
///
/// The page keeps every value in one draft so one Save cannot write only the
/// field that happened to have focus. The scrollback capacity is part of the
/// same edit because the page shows it beside the font fields.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalPageSettings {
    /// The CSS font-family chain, trimmed by the save.
    pub family: String,
    /// The font size in logical pixels.
    pub size: u16,
    /// The line-height multiplier the renderer uses.
    pub line_height: f64,
    /// Extra space between cells, in pixels.
    pub letter_spacing: i8,
    /// The scrollback capacity; `0` means unlimited.
    pub scrollback_lines: u32,
    /// The cursor shape: `block`, `bar` or `underline`.
    pub cursor_style: String,
    /// The left page padding in logical pixels.
    pub padding_left: u16,
    /// The right page padding in logical pixels.
    pub padding_right: u16,
    /// The bottom page padding in logical pixels.
    pub padding_bottom: u16,
}

impl From<&AppConfig> for TerminalPageSettings {
    fn from(config: &AppConfig) -> Self {
        Self {
            family: config.font_family.clone(),
            size: config.font_size,
            line_height: config.terminal_line_height,
            letter_spacing: config.terminal_letter_spacing,
            scrollback_lines: config.scrollback_lines,
            cursor_style: config.cursor_style.clone(),
            padding_left: config.terminal_padding_left_px,
            padding_right: config.terminal_padding_right_px,
            padding_bottom: config.terminal_padding_bottom_px,
        }
    }
}

impl TerminalPageSettings {
    /// Checks the values the settings page would save and normalizes the ones
    /// the file rounds, so a caller can refuse an invalid draft before it
    /// reaches a writer.
    pub fn checked(self) -> Result<Self, String> {
        checked_page(self)
    }
}

/// Replaces the Terminal page values in the shared config file as one edit.
///
/// `previous` is the snapshot the caller last loaded or saved. Only a field
/// that differs from it is written, so an unrelated setting, a key this
/// version does not know, or a page field another client changed since the
/// snapshot survives. An invalid draft or an unreadable config leaves the
/// file as it was, and the result carries the values the next launch will
/// load.
pub fn save_terminal_page(
    previous: &AppConfig,
    settings: TerminalPageSettings,
) -> Result<TerminalPageSettings, String> {
    save_page_at(
        &ensure_config_dir()?.join("config.json"),
        previous,
        settings,
    )
}

pub(super) fn save_page_at(
    path: &Path,
    previous: &AppConfig,
    settings: TerminalPageSettings,
) -> Result<TerminalPageSettings, String> {
    let checked = checked_page(settings)?;
    let previous = TerminalPageSettings::from(previous);
    let _edit = super::edit::lock()?;
    let mut config = super::load_config_at(path)?;
    if checked != previous {
        if checked.family != previous.family {
            config.font_family = checked.family;
        }
        if checked.size != previous.size {
            config.font_size = checked.size;
        }
        if checked.line_height != previous.line_height {
            config.terminal_line_height = checked.line_height;
        }
        if checked.letter_spacing != previous.letter_spacing {
            config.terminal_letter_spacing = checked.letter_spacing;
        }
        if checked.scrollback_lines != previous.scrollback_lines {
            config.scrollback_lines = checked.scrollback_lines;
        }
        if checked.cursor_style != previous.cursor_style {
            config.cursor_style = checked.cursor_style;
        }
        if checked.padding_left != previous.padding_left {
            config.terminal_padding_left_px = checked.padding_left;
        }
        if checked.padding_right != previous.padding_right {
            config.terminal_padding_right_px = checked.padding_right;
        }
        if checked.padding_bottom != previous.padding_bottom {
            config.terminal_padding_bottom_px = checked.padding_bottom;
        }
        super::compatible_file::save_at(path, &config)?;
        config = super::load_config_at(path)?;
    }
    Ok(TerminalPageSettings::from(&config))
}

fn checked_page(settings: TerminalPageSettings) -> Result<TerminalPageSettings, String> {
    let family = checked_family(&settings.family)?;
    checked_size(settings.size)?;
    if !settings.line_height.is_finite()
        || !TERMINAL_LINE_HEIGHT_RANGE.contains(&settings.line_height)
    {
        return Err(format!(
            "The line height {} is outside the {} to {} the settings accept.",
            settings.line_height,
            TERMINAL_LINE_HEIGHT_RANGE.start(),
            TERMINAL_LINE_HEIGHT_RANGE.end()
        ));
    }
    if !TERMINAL_LETTER_SPACING_RANGE.contains(&i16::from(settings.letter_spacing)) {
        return Err(format!(
            "The letter spacing {} is outside the {} to {} pixels the settings accept.",
            settings.letter_spacing,
            TERMINAL_LETTER_SPACING_RANGE.start(),
            TERMINAL_LETTER_SPACING_RANGE.end()
        ));
    }
    if settings.scrollback_lines > MAX_EXPLICIT_SCROLLBACK_LINES {
        return Err(format!(
            "The scrollback limit {} is larger than the {} lines the settings accept.",
            settings.scrollback_lines, MAX_EXPLICIT_SCROLLBACK_LINES
        ));
    }
    if !CURSOR_STYLES.contains(&settings.cursor_style.as_str()) {
        return Err(format!(
            "The cursor style '{}' is not one of block, bar or underline.",
            settings.cursor_style
        ));
    }
    checked_padding("left", settings.padding_left)?;
    checked_padding("right", settings.padding_right)?;
    checked_padding("bottom", settings.padding_bottom)?;
    Ok(TerminalPageSettings {
        family,
        size: settings.size,
        line_height: normalize_terminal_line_height(Some(settings.line_height)),
        letter_spacing: settings.letter_spacing,
        scrollback_lines: normalize_scrollback_config(Some(f64::from(settings.scrollback_lines))),
        cursor_style: settings.cursor_style,
        padding_left: settings.padding_left,
        padding_right: settings.padding_right,
        padding_bottom: settings.padding_bottom,
    })
}

fn checked_padding(side: &str, padding: u16) -> Result<(), String> {
    if padding <= MAX_PAGE_PADDING {
        return Ok(());
    }
    Err(format!(
        "The {side} padding {padding} is outside the 0 to {MAX_PAGE_PADDING} pixels the settings accept."
    ))
}

/// Replaces the Terminal font family and size in the shared config file.
///
/// The family is trimmed the way the settings form trims it and must name at
/// least one font without control characters. The size must be within
/// [`TERMINAL_FONT_SIZE_RANGE`], because both values reach the renderer
/// unchanged. An invalid edit or an unreadable config leaves the file as it was.
pub fn save_terminal_font(family: String, size: u16) -> Result<SavedTerminalFont, String> {
    save_at(&ensure_config_dir()?.join("config.json"), family, size)
}

pub(super) fn save_at(path: &Path, family: String, size: u16) -> Result<SavedTerminalFont, String> {
    let family = checked_family(&family)?;
    checked_size(size)?;
    let _edit = super::edit::lock()?;
    let mut config = super::load_config_at(path)?;
    config.font_family = family;
    config.font_size = size;
    super::compatible_file::save_at(path, &config)?;
    let reloaded = super::load_config_at(path)?;
    Ok(SavedTerminalFont {
        family: reloaded.font_family,
        size: reloaded.font_size,
    })
}

fn checked_family(family: &str) -> Result<String, String> {
    let family = js_trim(family);
    if family.is_empty() {
        return Err(
            "The terminal font cannot be empty. Choose a font or a CSS font-family chain."
                .to_string(),
        );
    }
    if family.chars().any(char::is_control) {
        return Err(format!(
            "The terminal font '{family}' contains a control character. Remove it before saving."
        ));
    }
    Ok(family.to_owned())
}

fn checked_size(size: u16) -> Result<(), String> {
    if TERMINAL_FONT_SIZE_RANGE.contains(&size) {
        return Ok(());
    }
    Err(format!(
        "The terminal font size {size} is outside the {} to {} pixels the settings accept.",
        TERMINAL_FONT_SIZE_RANGE.start(),
        TERMINAL_FONT_SIZE_RANGE.end()
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::AppConfig;
    use super::*;

    const FAMILY: &str = "\"JetBrains Mono\", Menlo, monospace";

    fn temp_config() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("config-{}.json", uuid::Uuid::new_v4()))
    }

    #[test]
    fn writes_the_font_into_the_latest_config_without_resetting_unrelated_settings() {
        let path = temp_config();
        super::super::compatible_file::save_at(
            &path,
            &AppConfig {
                scrollback_lines: 372,
                ..AppConfig::default()
            },
        )
        .unwrap();
        let extended =
            fs::read_to_string(&path)
                .unwrap()
                .replacen('{', r#"{"future":1.2300e+45,"#, 1);
        fs::write(&path, extended).unwrap();

        let saved = save_at(&path, format!("  {FAMILY}  "), 21).unwrap();

        assert_eq!(
            saved,
            SavedTerminalFont {
                family: FAMILY.to_string(),
                size: 21,
            }
        );
        let reloaded = super::super::load_config_at(&path).unwrap();
        assert_eq!(
            (reloaded.font_family.as_str(), reloaded.font_size),
            (FAMILY, 21)
        );
        assert_eq!(reloaded.scrollback_lines, 372);
        assert!(fs::read_to_string(&path).unwrap().contains("1.2300e+45"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_later_edit_keeps_the_previous_font_and_the_settings_around_it() {
        let path = temp_config();
        save_at(&path, FAMILY.to_string(), 18).unwrap();
        let mut without_font_edit = super::super::load_config_at(&path).unwrap();
        without_font_edit.scrollback_lines = 900;
        super::super::compatible_file::save_at(&path, &without_font_edit).unwrap();

        let saved = save_at(&path, "Menlo".to_string(), 22).unwrap();

        assert_eq!((saved.family.as_str(), saved.size), ("Menlo", 22),);
        let reloaded = super::super::load_config_at(&path).unwrap();
        assert_eq!(
            (
                reloaded.font_family.as_str(),
                reloaded.font_size,
                reloaded.scrollback_lines
            ),
            ("Menlo", 22, 900)
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn creates_a_complete_config_when_none_exists() {
        let path = temp_config();

        let saved = save_at(&path, FAMILY.to_string(), 6).unwrap();

        assert_eq!((saved.family.as_str(), saved.size), (FAMILY, 6));
        let reloaded = super::super::load_config_at(&path).unwrap();
        assert_eq!(
            (reloaded.font_family.as_str(), reloaded.font_size),
            (FAMILY, 6)
        );
        assert_eq!(
            reloaded.scrollback_lines,
            AppConfig::default().scrollback_lines
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn refuses_a_font_the_settings_cannot_use_without_touching_the_file() {
        let path = temp_config();
        super::super::compatible_file::save_at(&path, &AppConfig::default()).unwrap();
        let before = fs::read_to_string(&path).unwrap();

        for (family, size) in [
            (FAMILY.to_string(), 5),
            (FAMILY.to_string(), 73),
            (FAMILY.to_string(), 0),
            (String::new(), 14),
            (" \t ".to_string(), 14),
            ("Menlo\nmonospace".to_string(), 14),
            ("Menlo\u{7f}monospace".to_string(), 14),
        ] {
            assert!(
                save_at(&path, family.clone(), size).is_err(),
                "{family:?} at {size} pixels must be refused"
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), before);
        }

        let missing = temp_config();
        assert!(save_at(&missing, FAMILY.to_string(), 100).is_err());
        assert!(!missing.exists());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_an_unreadable_config() {
        let path = temp_config();
        for damaged in ["{", r#"{"theme":false}"#] {
            fs::write(&path, damaged).unwrap();
            assert!(save_at(&path, FAMILY.to_string(), 14).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), damaged);
        }
        fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod page_tests;
