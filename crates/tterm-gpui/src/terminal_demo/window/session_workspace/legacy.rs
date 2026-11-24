//! Reads the terminal content Tauri saved for the restored tabs.
//!
//! The files stay owned by the Tauri client: this module only reads them, and
//! the GPUI workspace save must not prune or overwrite them.

use std::collections::HashMap;
use std::path::Path;

use chrono::{Datelike, Local, TimeZone, Timelike};
use tokio::runtime::Handle;
use tterm_core::core::blocking::run_blocking;
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{ArgumentValue, TranslationRequest};
use tterm_core::session_restore::scrollback::{SavedScrollback, read_legacy_snapshot};

use crate::locale::Locale;
use crate::terminal_demo::session::RestoredHistory;

/// The decompressed content kept for all restored tabs together.
///
/// Each tab is read one at a time, so at most one more snapshot (bounded by
/// the reader's own per-file limit) is decoded transiently while this cap is
/// applied. The retained map is therefore bounded by this value, not by the
/// sum of every snapshot on disk; a tab past the cap starts without history.
const RETAINED_CONTENT_LIMIT: usize = 64 * 1024 * 1024;

/// Reads the snapshots of `tab_ids` off the GPUI thread.
pub(super) fn read_saved(runtime: &Handle, tab_ids: &[String]) -> HashMap<String, SavedScrollback> {
    let dir = match tterm_core::config::paths::get_config_path() {
        // The core snapshot module keeps its files beside the database.
        Ok(config) => config.join("scrollback"),
        Err(error) => {
            eprintln!("tterm-gpui: failed to locate saved terminal content: {error}");
            return HashMap::new();
        }
    };
    let tab_ids = tab_ids.to_vec();
    match runtime.block_on(run_blocking(move || Ok(read_bounded(&dir, &tab_ids)))) {
        Ok(saved) => saved,
        Err(error) => {
            eprintln!("tterm-gpui: failed to read saved terminal content: {error}");
            HashMap::new()
        }
    }
}

fn read_bounded(dir: &Path, tab_ids: &[String]) -> HashMap<String, SavedScrollback> {
    let mut retained = 0usize;
    let mut saved = HashMap::new();
    for tab_id in tab_ids {
        let Some(snapshot) = read_legacy_snapshot(dir, tab_id) else {
            continue;
        };
        let length = snapshot.content.len();
        if retained.saturating_add(length) > RETAINED_CONTENT_LIMIT {
            eprintln!(
                "tterm-gpui: skipping saved content for tab '{tab_id}': the retained content limit was reached"
            );
            continue;
        }
        retained += length;
        saved.insert(snapshot.tab_id.clone(), snapshot);
    }
    saved
}

/// Builds the model input for one tab, or `None` when it showed nothing.
pub(super) fn restored_history(
    locale: &Locale,
    snapshot: SavedScrollback,
) -> Option<RestoredHistory> {
    if snapshot.content.trim().is_empty() {
        return None;
    }
    let time = format_saved_at(snapshot.saved_at, chrono::Utc::now().timestamp_millis());
    let arguments = [("time", ArgumentValue::Text(time.as_str()))];
    let banner = locale
        .translate(
            TranslationRequest::new(keys::SESSION_HEADER_CONTENT_RESTORED).arguments(&arguments),
        )
        .unwrap_or_else(|error| {
            eprintln!("tterm-gpui: failed to translate the restore banner: {error}");
            format!("Restored · saved {time}")
        });
    Some(RestoredHistory {
        content: snapshot.content,
        banner,
    })
}

/// The Tauri client's `formatSavedAt`: month-day and 24-hour time, with the
/// year in front when it is not the current year.
fn format_saved_at(saved_at: i64, now: i64) -> String {
    let Some(date) = Local.timestamp_millis_opt(saved_at).single() else {
        return String::new();
    };
    let current_year = Local
        .timestamp_millis_opt(now)
        .single()
        .map(|current| current.year());
    let year = if current_year != Some(date.year()) {
        format!("{}-", date.year())
    } else {
        String::new()
    };
    format!(
        "{year}{:02}-{:02} {:02}:{:02}",
        date.month(),
        date.day(),
        date.hour(),
        date.minute()
    )
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};

    use super::format_saved_at;

    #[test]
    fn saved_at_uses_the_tauri_date_shape() {
        let now = Local
            .with_ymd_and_hms(2026, 11, 24, 9, 30, 0)
            .single()
            .unwrap()
            .timestamp_millis();
        let same_year = Local
            .with_ymd_and_hms(2026, 1, 2, 3, 4, 0)
            .single()
            .unwrap()
            .timestamp_millis();
        let earlier_year = Local
            .with_ymd_and_hms(2025, 10, 25, 14, 3, 0)
            .single()
            .unwrap()
            .timestamp_millis();

        assert_eq!(format_saved_at(same_year, now), "01-02 03:04");
        assert_eq!(format_saved_at(earlier_year, now), "2025-10-25 14:03");
    }
}
