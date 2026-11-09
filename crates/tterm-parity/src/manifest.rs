//! `parity/screens.json`: the list of states the reference captures.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The manifest schema this tool understands.
pub const MANIFEST_VERSION: u32 = 2;

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

/// The platform the reference side is told it runs on.
///
/// `plugin-os` is synchronous in the frontend, so parity mode answers it from
/// here instead of from the host. Every screen in a run sees the same values.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformSpec {
    /// `macos`, `windows`, `linux`, ...
    pub os: String,
    /// The OS version string.
    pub version: String,
    /// `unix` or `windows`.
    pub family: String,
    /// `aarch64`, `x86_64`, ...
    pub arch: String,
    /// `macos`, `windows`, `linux`, ...
    #[serde(rename = "type")]
    pub os_type: String,
    /// Empty on macOS; `.exe` on Windows.
    pub exe_extension: String,
    /// The end-of-line sequence.
    pub eol: String,
}

/// The whole manifest.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreensManifest {
    /// Schema version.
    pub version: u32,
    /// The window to capture at.
    pub viewport: ViewportSpec,
    #[serde(default)]
    pub windows_viewport: Option<ViewportSpec>,
    /// `dark` or `light`; the app's own theme naming.
    pub theme: String,
    /// Interface scale in percent, as the settings express it.
    pub ui_scale_percent: u32,
    /// What `plugin-os` answers in parity mode.
    pub platform: PlatformSpec,
    /// Optional platform overrides used by the Windows parity channel.
    #[serde(default)]
    pub platforms: std::collections::BTreeMap<String, PlatformSpec>,
    /// The screens, in report order.
    pub screens: Vec<ScreenSpec>,
}

impl ScreensManifest {
    pub fn selected_viewport(&self) -> ViewportSpec {
        if std::env::var("PARITY_PLATFORM").as_deref() == Ok("windows") {
            self.windows_viewport.unwrap_or(self.viewport)
        } else {
            self.viewport
        }
    }
}

impl ScreensManifest {
    /// Reads and validates `parity/screens.json` under `root`.
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = manifest_path(root);
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let mut manifest: Self = serde_json::from_str(&text)
            .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
        if let Ok(theme) = std::env::var("PARITY_THEME")
            && !theme.trim().is_empty()
        {
            manifest.theme = theme;
        }
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
        for (field, value) in [
            ("os", self.platform.os.as_str()),
            ("version", self.platform.version.as_str()),
            ("family", self.platform.family.as_str()),
            ("arch", self.platform.arch.as_str()),
            ("type", self.platform.os_type.as_str()),
            ("eol", self.platform.eol.as_str()),
        ] {
            if value.is_empty() {
                return Err(format!(
                    "parity/screens.json platform.{field} is empty; plugin-os answers it synchronously"
                ));
            }
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
