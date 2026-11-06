//! The fixture data a GPUI screen is built from.
//!
//! The reference side loads the same JSON through `src/parity/fixtures.ts`,
//! where TypeScript checks it against the type the real Tauri command returns.
//! This module only takes the values the example screen needs; Serde ignores
//! the rest, including the `screen` field the reference reads.

use serde::Deserialize;
use std::fs;
use std::path::Path;

/// One fixture file.
#[derive(Debug, Deserialize)]
pub struct FixtureFile {
    /// Fixture answers keyed by command name.
    #[serde(default)]
    pub commands: Commands,
}

/// The commands the W0-04 example screen reads.
#[derive(Debug, Default, Deserialize)]
pub struct Commands {
    /// `list_profiles`, the command the example screen calls on mount.
    #[serde(default)]
    pub list_profiles: Vec<ProfileSummary>,
}

/// The fields of a saved profile this tool reads.
#[derive(Debug, Deserialize)]
pub struct ProfileSummary {
    /// The profile's display name, shown in the example input.
    pub name: String,
}

impl FixtureFile {
    /// Reads the fixture at `path`.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("could not read fixture {}: {error}", path.display()))?;
        serde_json::from_str(&text)
            .map_err(|error| format!("could not parse fixture {}: {error}", path.display()))
    }

    /// The name the example input shows: the first profile's.
    pub fn first_profile_name(&self) -> Result<String, String> {
        self.commands
            .list_profiles
            .first()
            .map(|profile| profile.name.clone())
            .ok_or_else(|| {
                "the fixture's commands.list_profiles is missing or empty; the example screen \
                 needs one profile"
                    .to_owned()
            })
    }
}
