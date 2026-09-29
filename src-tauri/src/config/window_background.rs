//! The last theme background, remembered so the next launch paints its first
//! frame in it instead of a fixed color. Kept out of `config.json` because the
//! frontend rewrites that file from its own copy of the config.

use std::fs;

use super::{atomic_write, ensure_config_dir, get_config_path};

const FILE_NAME: &str = "window-background";

fn parse_hex_color(value: &str) -> Option<(u8, u8, u8)> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let channel = |range| u8::from_str_radix(&hex[range], 16).ok();
    Some((channel(0..2)?, channel(2..4)?, channel(4..6)?))
}

pub fn load_window_background() -> Option<(u8, u8, u8)> {
    let path = get_config_path().ok()?.join(FILE_NAME);
    parse_hex_color(&fs::read_to_string(path).ok()?)
}

#[tauri::command]
pub fn save_window_background(color: String) -> Result<(), String> {
    let (r, g, b) = parse_hex_color(&color).ok_or_else(|| format!("Invalid color: {color}"))?;
    if load_window_background() == Some((r, g, b)) {
        return Ok(());
    }
    let path = ensure_config_dir()?.join(FILE_NAME);
    atomic_write(&path, format!("#{r:02x}{g:02x}{b:02x}"))
}

#[cfg(test)]
mod tests {
    use super::parse_hex_color;

    #[test]
    fn parses_six_digit_hex() {
        assert_eq!(parse_hex_color("#1b1d23"), Some((0x1b, 0x1d, 0x23)));
        assert_eq!(parse_hex_color(" #FFFFFF\n"), Some((255, 255, 255)));
    }

    #[test]
    fn rejects_other_forms() {
        assert_eq!(parse_hex_color("1b1d23"), None);
        assert_eq!(parse_hex_color("#fff"), None);
        assert_eq!(parse_hex_color("#1b1d2g"), None);
        assert_eq!(parse_hex_color("#1b1dé"), None);
        assert_eq!(parse_hex_color(""), None);
    }
}
