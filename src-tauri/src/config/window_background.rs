//! The last theme background, remembered so the next launch paints its first
//! frame in it instead of a fixed color. Kept out of `config.json` because the
//! frontend rewrites that file from its own copy of the config; the database
//! is open before the window is built.

use crate::core::blocking::run_blocking;
use crate::db::meta;
use rusqlite::Connection;

const KEY: &str = "window_background";
/// Where older versions kept it.
const FILE_NAME: &str = "window-background";

fn parse_hex_color(value: &str) -> Option<(u8, u8, u8)> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let channel = |range| u8::from_str_radix(&hex[range], 16).ok();
    Some((channel(0..2)?, channel(2..4)?, channel(4..6)?))
}

pub(crate) fn read(connection: &Connection) -> Result<Option<(u8, u8, u8)>, String> {
    Ok(meta::get::<String>(connection, KEY)?.and_then(|color| parse_hex_color(&color)))
}

/// Saves `color` (`#rrggbb`) unless it is already saved.
fn write(connection: &Connection, color: &str) -> Result<(), String> {
    let (r, g, b) = parse_hex_color(color).ok_or_else(|| format!("Invalid color: {color}"))?;
    if read(connection)? != Some((r, g, b)) {
        meta::set(connection, KEY, &format!("#{r:02x}{g:02x}{b:02x}"))?;
    }
    Ok(())
}

/// `None` when none is saved or the database is unavailable.
pub fn load_window_background() -> Option<(u8, u8, u8)> {
    crate::db::read(read).ok().flatten()
}

#[tauri::command]
pub async fn save_window_background(color: String) -> Result<(), String> {
    run_blocking(move || crate::db::write(|transaction| write(transaction, &color))).await
}

/// Moves the file older versions kept into the database.
pub(crate) fn import_file(
    database: &crate::db::Database,
    config_dir: &std::path::Path,
) -> Result<(), String> {
    meta::import_file(
        database,
        &config_dir.join(FILE_NAME),
        |connection, bytes| {
            // An unreadable color only costs one first frame in the default.
            match parse_hex_color(&String::from_utf8_lossy(bytes)) {
                Some(_) if meta::contains(connection, KEY)? => Ok(()),
                Some(_) => write(connection, &String::from_utf8_lossy(bytes)),
                None => Ok(()),
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn colors_round_trip_normalized() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                assert_eq!(read(connection)?, None);
                write(
                    connection,
                    " #1B1D23
",
                )?;
                assert_eq!(read(connection)?, Some((0x1b, 0x1d, 0x23)));
                assert_eq!(
                    meta::get::<String>(connection, KEY)?.as_deref(),
                    Some("#1b1d23")
                );
                assert!(write(connection, "red").is_err());
                assert_eq!(read(connection)?, Some((0x1b, 0x1d, 0x23)));
                Ok(())
            })
            .unwrap();
    }

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
