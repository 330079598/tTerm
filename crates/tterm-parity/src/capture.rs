//! Renders every manifest screen with GPUI's headless renderer.
//!
//! The renderer is the same one `crates/tterm-gpui/examples/glyph_grid.rs` uses:
//! `HeadlessAppContext` with the real platform text system (CoreText on macOS)
//! and the Metal headless renderer. It produces the pixels the real window would
//! draw without needing screen-recording permission.

use crate::layout::LayoutSink;
use crate::manifest::ScreensManifest;
use crate::measure;
use crate::screens::{ScreenView, example};
use gpui_kit::{AppContext, HeadlessAppContext, px, size};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One captured screen.
#[derive(Clone, Debug)]
pub struct CapturedScreen {
    /// Screen id from the manifest.
    pub id: String,
    /// PNG path under the output directory.
    pub image: PathBuf,
    /// Layout JSON path under the output directory.
    pub layout: PathBuf,
    /// Number of marked elements recorded.
    pub element_count: usize,
}

/// Captures every screen into `output`, overwriting previous files.
pub fn capture_all(root: &Path, output: &Path) -> Result<Vec<CapturedScreen>, String> {
    let manifest = ScreensManifest::load(root)?;
    fs::create_dir_all(output)
        .map_err(|error| format!("could not create {}: {error}", output.display()))?;
    let text_system = gpui_kit::platform::current_platform(true).text_system();
    let viewport = example::viewport(&manifest);

    let mut captured = Vec::with_capacity(manifest.screens.len());
    for screen in &manifest.screens {
        let sink = LayoutSink::new(&screen.id, viewport);
        let view = ScreenView::build(root, screen, &manifest, sink.clone())?;
        let mut context =
            HeadlessAppContext::with_platform(text_system.clone(), Arc::new(()), || {
                gpui_kit::platform::current_headless_renderer()
            });
        let window = context
            .open_window(
                size(
                    px(manifest.viewport.width as f32),
                    px(manifest.viewport.height as f32),
                ),
                |_, cx| cx.new(|_| view),
            )
            .map_err(|error| format!("could not open the {} window: {error}", screen.id))?;
        context.run_until_parked();
        let image = context
            .capture_screenshot(window.into())
            .map_err(|error| format!("could not capture {}: {error}", screen.id))?;

        let mut document = sink.finish();
        measure::measure_document(&image, &mut document)
            .map_err(|error| format!("could not measure {}: {error}", screen.id))?;
        if document.elements.is_empty() {
            return Err(format!(
                "{} recorded no elements; its parity markers did not paint",
                screen.id
            ));
        }
        let expected_width =
            (manifest.viewport.width as f32 * manifest.viewport.device_scale_factor).round() as u32;
        let expected_height = (manifest.viewport.height as f32
            * manifest.viewport.device_scale_factor)
            .round() as u32;
        if image.width() != expected_width || image.height() != expected_height {
            return Err(format!(
                "{} rendered at {}x{}, expected {}x{} (viewport {}x{} at {}x)",
                screen.id,
                image.width(),
                image.height(),
                expected_width,
                expected_height,
                manifest.viewport.width,
                manifest.viewport.height,
                manifest.viewport.device_scale_factor
            ));
        }

        let image_path = output.join(format!("{}.png", screen.id));
        let layout_path = output.join(format!("{}.layout.json", screen.id));
        image
            .save(&image_path)
            .map_err(|error| format!("could not write {}: {error}", image_path.display()))?;
        document.write(&layout_path)?;
        captured.push(CapturedScreen {
            id: screen.id.clone(),
            image: image_path,
            layout: layout_path,
            element_count: document.elements.len(),
        });
    }
    Ok(captured)
}
