//! Size and time labels the SFTP drawer shows, from
//! `src/components/SftpDrawer/sftpDrawerUtils.ts`.

use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};

use crate::js::to_fixed;

/// The largest remote file the editable-file view opens.
pub const MAX_EDIT_FILE_BYTES: u64 = 5 * 1024 * 1024;

/// The size labels the drawer shows; an unknown size is `--`.
pub fn format_bytes(value: Option<u64>) -> String {
    let Some(value) = value else {
        return "--".to_owned();
    };
    if value < 1024 {
        return format!("{value} B");
    }

    let units = ["KB", "MB", "GB", "TB"];
    let mut size = value as f64 / 1024.0;
    let mut index = 0;
    while size >= 1024.0 && index < units.len() - 1 {
        size /= 1024.0;
        index += 1;
    }
    let digits = if size >= 10.0 { 0 } else { 1 };
    format!("{} {}", to_fixed(size, digits), units[index])
}

/// Formats a local date and time the way the drawer does.
pub fn format_timestamp_at(value: DateTime<Local>) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        value.year(),
        value.month(),
        value.day(),
        value.hour(),
        value.minute(),
        value.second()
    )
}

/// The local time at `value` milliseconds since the Unix epoch.
pub fn format_timestamp(value: Option<i64>) -> String {
    let Some(value) = value.filter(|value| *value != 0) else {
        return "--".to_owned();
    };
    Local
        .timestamp_millis_opt(value)
        .single()
        .map(format_timestamp_at)
        .unwrap_or_else(|| "--".to_owned())
}
