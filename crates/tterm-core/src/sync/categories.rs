//! The categories a sync reports, and the events the client uses to refresh
//! the views showing them, from `src/lib/sync.ts`.
//!
//! Running the Tauri command and dispatching browser events stay in the
//! WebView adapter.

/// Collections the backend syncs; a completed sync lists the ones it wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncCategory {
    Profiles,
    ProfileGroups,
    Tunnels,
    Proxies,
    Secrets,
    Commands,
    KnownHosts,
    Settings,
    Themes,
    SerialSends,
}

/// Every category in the order the client declares them.
pub const SYNC_CATEGORIES: [SyncCategory; 10] = [
    SyncCategory::Profiles,
    SyncCategory::ProfileGroups,
    SyncCategory::Tunnels,
    SyncCategory::Proxies,
    SyncCategory::Secrets,
    SyncCategory::Commands,
    SyncCategory::KnownHosts,
    SyncCategory::Settings,
    SyncCategory::Themes,
    SyncCategory::SerialSends,
];

impl SyncCategory {
    /// The name used by the command result.
    pub fn as_str(self) -> &'static str {
        match self {
            SyncCategory::Profiles => "profiles",
            SyncCategory::ProfileGroups => "profileGroups",
            SyncCategory::Tunnels => "tunnels",
            SyncCategory::Proxies => "proxies",
            SyncCategory::Secrets => "secrets",
            SyncCategory::Commands => "commands",
            SyncCategory::KnownHosts => "knownHosts",
            SyncCategory::Settings => "settings",
            SyncCategory::Themes => "themes",
            SyncCategory::SerialSends => "serialSends",
        }
    }

    /// The category a command-result name identifies.
    pub fn parse(value: &str) -> Option<Self> {
        SYNC_CATEGORIES
            .into_iter()
            .find(|category| category.as_str() == value)
    }
}

/// How often local changes are looked for and uploaded.
pub const SYNC_PUSH_INTERVALS_SECS: [u32; 4] = [30, 60, 300, 900];
/// How often the server is checked for other devices' changes.
pub const SYNC_PULL_INTERVALS_MINS: [u32; 4] = [5, 15, 30, 60];

/// Dispatched by the WebView after the sync settings were saved.
pub const SYNC_SETTINGS_EVENT: &str = "tterm:sync-settings";
/// Dispatched by the WebView after a sync wrote local data.
pub const SYNC_APPLIED_EVENT: &str = "tterm:sync-applied";
/// Dispatched by the WebView after every sync attempt.
pub const SYNC_STATUS_EVENT: &str = "tterm:sync-status";

/// Whether a completed sync changed any category a view watches.
pub fn changed_any(changed: &[SyncCategory], watched: &[SyncCategory]) -> bool {
    changed.iter().any(|category| watched.contains(category))
}

#[cfg(test)]
mod tests;
