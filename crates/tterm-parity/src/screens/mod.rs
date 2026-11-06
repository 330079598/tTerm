//! The GPUI views the parity capture can render.
//!
//! There is one entry per screen id in `parity/screens.json`. The reference
//! side keeps its own registry in `src/parity/screens.ts`; a screen missing
//! from either side fails the manifest test or the capture, not silently.

pub mod example;

use crate::layout::LayoutSink;
use crate::manifest::{ScreenSpec, ScreensManifest};
use example::ExampleScreen;
use gpui_kit::{Context, IntoElement, Render, Window};
use std::path::Path;

/// A screen's GPUI view.
pub enum ScreenView {
    /// The W0-04 example: one button and one input.
    Example(ExampleScreen),
}

impl ScreenView {
    /// Builds the view for `screen`, loading its fixture.
    pub fn build(
        root: &Path,
        screen: &ScreenSpec,
        manifest: &ScreensManifest,
        sink: LayoutSink,
    ) -> Result<Self, String> {
        match screen.id.as_str() {
            example::SCREEN_ID => Ok(Self::Example(ExampleScreen::load(
                root, screen, manifest, sink,
            )?)),
            other => Err(format!(
                "No GPUI view is registered for parity screen \"{other}\""
            )),
        }
    }
}

impl Render for ScreenView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        match self {
            Self::Example(view) => view.render().into_any_element(),
        }
    }
}
