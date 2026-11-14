//! Searching the settings pages, from
//! `src/components/SettingsDialog/settingsSearch.ts`.
//!
//! The settings index itself (which page holds which translated title) stays
//! with the settings page, and revealing a result needs the DOM; this module
//! ranks the translated items a page supplies.

pub mod search;
pub mod search_index;
#[cfg(test)]
mod tests;
