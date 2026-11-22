//! Display formatting shared by the tunnel page cards.

use std::time::{SystemTime, UNIX_EPOCH};

use tterm_core::i18n::language::Language;

pub(super) fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let value = bytes as f64;
    let exponent = ((value.ln() / 1024.0_f64.ln()).floor() as usize).min(UNITS.len() - 1);
    let scaled = value / 1024.0_f64.powi(exponent as i32);
    if exponent == 0 || scaled >= 100.0 {
        format!("{:.0} {}", scaled.round(), UNITS[exponent])
    } else {
        format!("{scaled:.1} {}", UNITS[exponent])
    }
}

pub(super) fn format_uptime(timestamp: u64, now: u64) -> String {
    let seconds = now.saturating_sub(timestamp) / 1000;
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m {}s", seconds % 60);
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h {}m", minutes % 60);
    }
    format!("{}d {}h", hours / 24, hours % 24)
}

pub(super) fn format_date(unix_ms: u64, language: Language) -> String {
    let days = (unix_ms / 86_400_000) as i64;
    let (year, month, day) = civil_date(days);
    match language {
        Language::En => format!("{year}-{month:02}-{day:02}"),
        Language::Zh => format!("{year}/{month:02}/{day:02}"),
    }
}

fn civil_date(days_since_epoch: i64) -> (i64, i64, i64) {
    // Howard Hinnant's civil-from-days algorithm, valid across the Unix range.
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    (year, month, day)
}

pub(super) fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}
