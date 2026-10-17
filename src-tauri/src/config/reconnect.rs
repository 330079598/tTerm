//! The automatic SSH reconnect settings, read on every connect.

use std::fs;
use std::sync::RwLock;
use std::time::SystemTime;

use super::paths::get_config_path;
use super::{default_reconnect_enabled, default_reconnect_max_attempts, load_config_file};

/// Resolved reconnect settings, cached against the config file's mtime: every
/// session normalizes its connection on create, and re-reading + re-parsing the
/// whole config from disk each time would add needless disk I/O to connect.
static RECONNECT_SETTINGS_CACHE: RwLock<Option<(Option<SystemTime>, bool, u32)>> =
    RwLock::new(None);

/// Cache lookup: a hit requires the whole mtime token — including `None`
/// ("config file absent") — to match, so a machine running on defaults still
/// hits the cache instead of stat-ing the disk on every connect.
fn reconnect_settings_cache_hit(
    cached: Option<(Option<SystemTime>, bool, u32)>,
    mtime: Option<SystemTime>,
) -> Option<(bool, u32)> {
    cached
        .filter(|(cached_mtime, _, _)| *cached_mtime == mtime)
        .map(|(_, enabled, max_attempts)| (enabled, max_attempts))
}

/// Resolve the automatic SSH reconnect settings (enabled, max attempts).
/// Falls back to the built-in defaults when the config file is missing or
/// unreadable.
pub fn resolve_reconnect_settings() -> (bool, u32) {
    let mtime = get_config_path()
        .ok()
        .map(|dir| dir.join("config.json"))
        .and_then(|path| fs::metadata(path).ok())
        .and_then(|metadata| metadata.modified().ok());

    if let Some(cached) = RECONNECT_SETTINGS_CACHE
        .read()
        .ok()
        .and_then(|guard| reconnect_settings_cache_hit(*guard, mtime))
    {
        return cached;
    }

    let settings = load_config_file()
        .map(|config| (config.reconnect_enabled, config.reconnect_max_attempts))
        .unwrap_or_else(|_| {
            (
                default_reconnect_enabled(),
                default_reconnect_max_attempts(),
            )
        });

    if let Ok(mut cache) = RECONNECT_SETTINGS_CACHE.write() {
        *cache = Some((mtime, settings.0, settings.1));
    }
    settings
}
