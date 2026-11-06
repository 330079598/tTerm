//! `parity/screens.json`: the list of states the reference captures.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The manifest schema this tool understands.
pub const MANIFEST_VERSION: u32 = 1;

/// The window every screen is captured at.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewportSpec {
    /// Logical width in CSS pixels.
    pub width: u32,
    /// Logical height in CSS pixels.
    pub height: u32,
    /// Physical pixels per logical pixel.
    pub device_scale_factor: f32,
}

/// One state to compare.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenSpec {
    /// `<feature>.<view>.<state>`, the id both sides use.
    pub id: String,
    /// Human-readable name for the report.
    pub title: String,
    /// Fixture file under `parity/fixtures/`.
    pub fixture: String,
}

/// The whole manifest.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreensManifest {
    /// Schema version.
    pub version: u32,
    /// The window to capture at.
    pub viewport: ViewportSpec,
    /// `dark` or `light`; the app's own theme naming.
    pub theme: String,
    /// Interface scale in percent, as the settings express it.
    pub ui_scale_percent: u32,
    /// The screens, in report order.
    pub screens: Vec<ScreenSpec>,
}

impl ScreensManifest {
    /// Reads and validates `parity/screens.json` under `root`.
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = manifest_path(root);
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let manifest: Self = serde_json::from_str(&text)
            .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// The fixture path of one screen.
    pub fn fixture_path(root: &Path, screen: &ScreenSpec) -> PathBuf {
        root.join("parity").join("fixtures").join(&screen.fixture)
    }

    /// Rejects a manifest the tool cannot compare consistently.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != MANIFEST_VERSION {
            return Err(format!(
                "parity/screens.json has schema {}, expected {MANIFEST_VERSION}",
                self.version
            ));
        }
        if self.viewport.width == 0 || self.viewport.height == 0 {
            return Err("parity/screens.json has a zero viewport".to_owned());
        }
        if !self.viewport.device_scale_factor.is_finite()
            || self.viewport.device_scale_factor <= 0.0
        {
            return Err("parity/screens.json has a non-positive deviceScaleFactor".to_owned());
        }
        if self.theme != "dark" && self.theme != "light" {
            return Err(format!(
                "parity/screens.json theme is \"{}\"; expected dark or light",
                self.theme
            ));
        }
        if self.ui_scale_percent != 100 {
            return Err(format!(
                "parity/screens.json uiScalePercent is {}; the W0-04 example screen only supports 100",
                self.ui_scale_percent
            ));
        }
        if self.screens.is_empty() {
            return Err("parity/screens.json lists no screens".to_owned());
        }
        let mut ids = BTreeSet::new();
        for screen in &self.screens {
            if screen.id.trim().is_empty() {
                return Err("parity/screens.json has a screen with an empty id".to_owned());
            }
            if !ids.insert(screen.id.clone()) {
                return Err(format!("parity/screens.json lists \"{}\" twice", screen.id));
            }
            if screen.fixture.trim().is_empty() {
                return Err(format!(
                    "parity/screens.json screen \"{}\" has no fixture",
                    screen.id
                ));
            }
            if screen.fixture.contains('/') || screen.fixture.contains('\\') {
                return Err(format!(
                    "parity/screens.json screen \"{}\" has a fixture path; put the file name only",
                    screen.id
                ));
            }
        }
        Ok(())
    }
}

/// The path of the manifest inside the repository.
pub fn manifest_path(root: &Path) -> PathBuf {
    root.join("parity").join("screens.json")
}

#[cfg(test)]
mod tests;
