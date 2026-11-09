//! The layout document both sides export.
//!
//! Playwright's `collectLayout` in `scripts/parity/layout.mjs` writes the same
//! JSON shape from the DOM. The field names here are the contract; changing one
//! means changing both producers and the comparison.

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::rc::Rc;

/// Bumped whenever the JSON shape changes incompatibly.
pub const SCHEMA_VERSION: u32 = 3;

/// The window and pixel density a screen was captured at.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewportSnapshot {
    /// Logical width in CSS pixels.
    pub width: u32,
    /// Logical height in CSS pixels.
    pub height: u32,
    /// Physical pixels per logical pixel.
    pub device_scale_factor: f32,
}

/// A rectangle in logical pixels, relative to the window's top-left corner.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundsSnapshot {
    /// Distance from the left edge.
    pub x: f32,
    /// Distance from the top edge.
    pub y: f32,
    /// Width in logical pixels.
    pub width: f32,
    /// Height in logical pixels.
    pub height: f32,
}

impl BoundsSnapshot {
    /// Builds a rectangle from its four components.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// How much a bounds change is allowed to matter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElementSizing {
    /// A container, button box or icon whose size the text cannot change.
    Fixed,
    /// A label or a button that grows with its text.
    Text,
}

/// The padding of one element, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaddingSnapshot {
    /// Top padding.
    pub top: f32,
    /// Right padding.
    pub right: f32,
    /// Bottom padding.
    pub bottom: f32,
    /// Left padding.
    pub left: f32,
}

impl PaddingSnapshot {
    /// All four sides equal, as `p-*` sets them.
    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Horizontal and vertical padding, as `px-* py-*` set them.
    pub const fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

/// The style values section 5.2 compares exactly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleSnapshot {
    /// Background as 8-bit sRGB, measured from the rendered pixels.
    pub background_color: [u8; 4],
    /// Border as 8-bit sRGB, measured from the rendered pixels.
    pub border_color: [u8; 4],
    /// Border width in logical pixels.
    pub border_width: f32,
    /// Corner radius in logical pixels.
    pub border_radius: f32,
    /// Zero-offset, zero-blur outer shadow spread; zero when absent.
    pub ring_spread: f32,
    /// Canonical family; the platform UI font is `system-ui` on both sides.
    pub font_family: String,
    /// Font size in logical pixels.
    pub font_size: f32,
    /// Numeric font weight, e.g. 400 or 500.
    pub font_weight: u16,
    /// Line height in logical pixels.
    pub line_height: f32,
    /// Text color as 8-bit sRGB.
    pub color: [u8; 4],
    pub opacity: f32,
    /// Padding in logical pixels.
    pub padding: PaddingSnapshot,
    /// The background the view declared before measurement. Keeping it lets an
    /// opaque declaration be cross-checked against the pixels the renderer
    /// actually painted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_background_color: Option<[u8; 4]>,
    /// The border the view declared before measurement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_border_color: Option<[u8; 4]>,
}

/// One `data-parity` element or one GPUI parity marker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementSnapshot {
    /// The shared id, e.g. `example.button`.
    pub id: String,
    /// Whether text may change its size.
    pub sizing: ElementSizing,
    /// Border box relative to the window.
    pub bounds: BoundsSnapshot,
    /// The compared style values.
    pub style: StyleSnapshot,
}

/// One screen's exported layout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutDocument {
    /// Schema version; see [`SCHEMA_VERSION`].
    pub version: u32,
    /// The screen id from `parity/screens.json`.
    pub screen: String,
    /// The window this layout belongs to.
    pub viewport: ViewportSnapshot,
    /// Marked elements, sorted by id.
    pub elements: Vec<ElementSnapshot>,
    /// Text rectangles the masked SSIM leaves out.
    pub text_regions: Vec<BoundsSnapshot>,
}

impl LayoutDocument {
    /// Reads and validates one layout JSON.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let document: Self = serde_json::from_str(&text)
            .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
        if document.version != SCHEMA_VERSION {
            return Err(format!(
                "{} has layout schema {}, expected {SCHEMA_VERSION}",
                path.display(),
                document.version
            ));
        }
        Ok(document)
    }

    /// Writes the document as stable, pretty JSON with a trailing newline.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| format!("could not serialize layout: {error}"))?;
        fs::write(path, format!("{text}\n"))
            .map_err(|error| format!("could not write {}: {error}", path.display()))
    }
}

/// The collector a GPUI screen fills while it paints.
///
/// The marker wrapper records the bounds of its child on every prepaint, so a
/// later frame overwrites an earlier one instead of appending. Sorting happens
/// in [`LayoutSink::finish`], which is what makes the output deterministic.
#[derive(Clone)]
pub struct LayoutSink(Rc<RefCell<LayoutSinkState>>);

#[derive(Default)]
struct LayoutSinkState {
    screen: String,
    viewport: ViewportSnapshot,
    elements: BTreeMap<String, ElementSnapshot>,
    text_regions: BTreeMap<String, BoundsSnapshot>,
}

impl LayoutSink {
    /// Creates a sink for one screen.
    pub fn new(screen: impl Into<String>, viewport: ViewportSnapshot) -> Self {
        Self(Rc::new(RefCell::new(LayoutSinkState {
            screen: screen.into(),
            viewport,
            elements: BTreeMap::new(),
            text_regions: BTreeMap::new(),
        })))
    }

    /// Records or replaces one marked element.
    pub fn record_element(
        &self,
        id: &str,
        sizing: ElementSizing,
        bounds: BoundsSnapshot,
        style: StyleSnapshot,
    ) {
        self.0.borrow_mut().elements.insert(
            id.to_owned(),
            ElementSnapshot {
                id: id.to_owned(),
                sizing,
                bounds,
                style,
            },
        );
    }

    /// Records or replaces one text rectangle, keyed by a caller-chosen name.
    pub fn record_text_region(&self, key: &str, bounds: BoundsSnapshot) {
        self.0
            .borrow_mut()
            .text_regions
            .insert(key.to_owned(), bounds);
    }

    /// Takes a consistent snapshot of everything recorded so far.
    pub fn finish(&self) -> LayoutDocument {
        let state = self.0.borrow();
        LayoutDocument {
            version: SCHEMA_VERSION,
            screen: state.screen.clone(),
            viewport: state.viewport,
            elements: state.elements.values().cloned().collect(),
            text_regions: state.text_regions.values().copied().collect(),
        }
    }
}

impl Default for ViewportSnapshot {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            device_scale_factor: 1.0,
        }
    }
}

#[cfg(test)]
mod tests;
