//! Parsers for the font draft fields, shared with the validation tests.
//!
//! Each parser returns the catalogue key the section shows when the value is
//! missing, malformed or outside the range the shared config accepts.

use tterm_core::config::limits::{
    TERMINAL_FONT_SIZE_RANGE, TERMINAL_LETTER_SPACING_RANGE, TERMINAL_LINE_HEIGHT_RANGE,
};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;

/// Parses the custom size field, returning the message key to show when it is
/// not one of the sizes the shared config accepts.
pub(super) fn parse_size(value: &str) -> Result<u16, TranslationKey> {
    let value = value.trim();
    if value.is_empty() {
        return Err(keys::FONT_SETTINGS_SIZE_REQUIRED);
    }
    let size = value
        .parse::<u16>()
        .map_err(|_| keys::FONT_SETTINGS_SIZE_INVALID)?;
    if !TERMINAL_FONT_SIZE_RANGE.contains(&size) {
        return Err(keys::FONT_SETTINGS_SIZE_RANGE);
    }
    Ok(size)
}

/// Parses the line-height field and rounds it to the two decimals the shared
/// settings file stores.
pub(super) fn parse_line_height(value: &str) -> Result<f64, TranslationKey> {
    let value = value.trim();
    if value.is_empty() {
        return Err(keys::FONT_SETTINGS_LINE_HEIGHT_REQUIRED);
    }
    let value = value
        .parse::<f64>()
        .map_err(|_| keys::FONT_SETTINGS_LINE_HEIGHT_INVALID)?;
    if !value.is_finite() {
        return Err(keys::FONT_SETTINGS_LINE_HEIGHT_INVALID);
    }
    if !TERMINAL_LINE_HEIGHT_RANGE.contains(&value) {
        return Err(keys::FONT_SETTINGS_LINE_HEIGHT_RANGE);
    }
    Ok((value * 100.0).round() / 100.0)
}

/// Parses the letter-spacing field, which is a whole number of pixels.
pub(super) fn parse_letter_spacing(value: &str) -> Result<i8, TranslationKey> {
    let value = value.trim();
    if value.is_empty() {
        return Err(keys::FONT_SETTINGS_LETTER_SPACING_REQUIRED);
    }
    let value = value
        .parse::<i16>()
        .map_err(|_| keys::FONT_SETTINGS_LETTER_SPACING_INVALID)?;
    if !TERMINAL_LETTER_SPACING_RANGE.contains(&value) {
        return Err(keys::FONT_SETTINGS_LETTER_SPACING_RANGE);
    }
    Ok(value as i8)
}

/// Whether a font name belongs to the Nerd Font family of patched builds.
pub(super) fn is_nerd_font(font: &str) -> bool {
    let font = font.to_lowercase();
    font.contains("nerd")
        || font.contains("powerline")
        || font
            .split(|character: char| !character.is_alphanumeric())
            .any(|token| token == "nf")
}
