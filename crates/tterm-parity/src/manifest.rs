//! `parity/screens.json`: the list of states the reference captures.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The manifest schema this tool understands.
pub const MANIFEST_VERSION: u32 = 2;

fn default_ui_style() -> String {
    "classic".to_owned()
}

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
    /// Optional style override for this screen (`classic` or `islands`).
    #[serde(default)]
    pub ui_style: Option<String>,
    /// Optional list limiting the styles captured for this screen.
    #[serde(default)]
    pub ui_styles: Option<Vec<String>>,
    /// Optional scale override for this screen.
    #[serde(default)]
    pub ui_scale_percent: Option<u32>,
    /// Optional list limiting the scales captured for this screen.
    #[serde(default)]
    pub ui_scale_percents: Option<Vec<u32>>,
    /// Actions executed by the reference capture before the screenshot.
    #[serde(default)]
    pub actions: Vec<ActionSpec>,
    /// Regions registered in `parity/exceptions.md` (ui-parity 5.2): they are
    /// left out of the image comparison and judged by a person instead.
    #[serde(default)]
    pub exceptions: Vec<ExceptionSpec>,
}

/// One registered exception on a screen.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionSpec {
    /// The entry heading in `parity/exceptions.md`, such as `E-001`.
    pub entry: String,
    /// Marked elements whose surroundings the entry covers.
    pub elements: Vec<String>,
    /// How far outside each element's border box the region reaches, in
    /// logical pixels. The element itself is still compared.
    pub outset: f32,
}

/// A deterministic browser action declared by a screen.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ActionSpec {
    /// Click a marked element or action target.
    Click { target: String },
    /// Move the pointer over a marked element or action target.
    Hover { target: String },
    /// Press a key, optionally with modifiers.
    Key {
        key: String,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    /// Drag one marked element or action target to another.
    Drag { from: String, to: String },
    /// Wait until a marked element or action target exists.
    WaitFor { target: String },
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
    /// Default window style used when no list is supplied.
    #[serde(default = "default_ui_style")]
    pub ui_style: String,
    /// Styles to capture globally. A screen may narrow this list.
    #[serde(default)]
    pub ui_styles: Option<Vec<String>>,
    /// Scales to capture globally. A screen may narrow this list.
    #[serde(default)]
    pub ui_scale_percents: Option<Vec<u32>>,
    /// What `plugin-os` answers in parity mode.
    pub platform: PlatformSpec,
    /// Optional platform overrides used by the Windows parity channel.
    #[serde(default)]
    pub platforms: std::collections::BTreeMap<String, PlatformSpec>,
    /// The screens, in report order.
    pub screens: Vec<ScreenSpec>,
}

impl ScreensManifest {
    /// Returns the Cartesian product of the style and scale dimensions for a screen.
    pub fn variants(&self, screen: &ScreenSpec) -> Vec<(String, u32)> {
        let styles: Vec<String> = screen
            .ui_styles
            .clone()
            .or_else(|| self.ui_styles.clone())
            .or_else(|| screen.ui_style.clone().map(|style| vec![style]))
            .unwrap_or_else(|| vec![self.ui_style.clone()]);
        let scales: Vec<u32> = screen
            .ui_scale_percents
            .clone()
            .or_else(|| self.ui_scale_percents.clone())
            .or_else(|| screen.ui_scale_percent.map(|scale| vec![scale]))
            .unwrap_or_else(|| vec![self.ui_scale_percent]);
        styles
            .iter()
            .flat_map(|style| scales.iter().map(move |scale| (style.clone(), *scale)))
            .collect()
    }

    pub fn selected_viewport(&self) -> ViewportSpec {
        if std::env::var("PARITY_PLATFORM").as_deref() == Ok("windows") {
            self.windows_viewport.unwrap_or(self.viewport)
        } else {
            self.viewport
        }
    }

    /// The style selected for this process, after checking the manifest list.
    pub fn selected_ui_style(&self) -> Result<String, String> {
        let style = std::env::var("PARITY_UI_STYLE").unwrap_or_else(|_| self.ui_style.clone());
        let allowed = self
            .ui_styles
            .as_deref()
            .unwrap_or(std::slice::from_ref(&self.ui_style));
        if !allowed.iter().any(|value| value == &style) {
            return Err(format!(
                "ui_style \"{style}\" is not enabled in parity/screens.json"
            ));
        }
        Ok(style)
    }

    /// The scale selected for this process, after checking the manifest list.
    pub fn selected_ui_scale(&self) -> Result<u32, String> {
        let scale = std::env::var("PARITY_UI_SCALE")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(self.ui_scale_percent);
        let allowed = self
            .ui_scale_percents
            .as_deref()
            .unwrap_or(std::slice::from_ref(&self.ui_scale_percent));
        if !allowed.contains(&scale) {
            return Err(format!(
                "ui_scale {scale} is not enabled in parity/screens.json"
            ));
        }
        Ok(scale)
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
        manifest.validate_exception_entries(root)?;
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
            validate_scale(self.ui_scale_percent)?;
        }
        validate_style(&self.ui_style)?;
        if let Some(styles) = &self.ui_styles {
            validate_styles(styles)?;
        }
        if let Some(scales) = &self.ui_scale_percents {
            for scale in scales {
                validate_scale(*scale)?;
            }
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
            if let Some(style) = &screen.ui_style {
                validate_style(style)?;
            }
            if let Some(styles) = &screen.ui_styles {
                validate_styles(styles)?;
            }
            if let Some(scale) = screen.ui_scale_percent {
                validate_scale(scale)?;
            }
            if let Some(scales) = &screen.ui_scale_percents {
                for scale in scales {
                    validate_scale(*scale)?;
                }
            }
            for exception in &screen.exceptions {
                if exception.entry.trim().is_empty() || exception.elements.is_empty() {
                    return Err(format!(
                        "parity/screens.json screen \"{}\" has an exception without an entry or elements",
                        screen.id
                    ));
                }
                if !exception.outset.is_finite() || exception.outset <= 0.0 {
                    return Err(format!(
                        "parity/screens.json screen \"{}\" exception {} needs a positive outset",
                        screen.id, exception.entry
                    ));
                }
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

fn validate_style(style: &str) -> Result<(), String> {
    if style != "classic" && style != "islands" {
        return Err(format!(
            "parity uiStyle is \"{style}\"; expected classic or islands"
        ));
    }
    Ok(())
}

fn validate_styles(styles: &[String]) -> Result<(), String> {
    if styles.is_empty() {
        return Err("parity uiStyles must not be empty".to_owned());
    }
    for style in styles {
        validate_style(style)?;
    }
    Ok(())
}

fn validate_scale(scale: u32) -> Result<(), String> {
    if scale != 100 && scale != 125 {
        return Err(format!(
            "parity uiScalePercent is {scale}; expected 100 or 125"
        ));
    }
    Ok(())
}

impl ScreensManifest {
    /// Every exception must name an entry that `parity/exceptions.md` registers
    /// as a `## <entry>` heading, so no region leaves the comparison unrecorded.
    fn validate_exception_entries(&self, root: &Path) -> Result<(), String> {
        let entries: Vec<&ExceptionSpec> = self
            .screens
            .iter()
            .flat_map(|screen| &screen.exceptions)
            .collect();
        if entries.is_empty() {
            return Ok(());
        }
        let path = root.join("parity").join("exceptions.md");
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let headings: BTreeSet<&str> = text
            .lines()
            .filter_map(|line| line.strip_prefix("## "))
            .filter_map(|heading| heading.split_whitespace().next())
            .collect();
        for exception in entries {
            if !headings.contains(exception.entry.as_str()) {
                return Err(format!(
                    "parity/screens.json uses exception {}, which parity/exceptions.md does not register",
                    exception.entry
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
