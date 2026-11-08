//! Renders every manifest screen with GPUI's headless renderer.
//!
//! The renderer is the same one `crates/tterm-gpui/examples/glyph_grid.rs` uses:
//! `HeadlessAppContext` with the real platform text system (CoreText on macOS)
//! and the Metal headless renderer. It produces the pixels the real window would
//! draw without needing screen-recording permission.

use crate::layout::LayoutSink;
use crate::manifest::ScreensManifest;
use crate::measure;
use crate::screens::ScreenView;
#[cfg(not(target_os = "windows"))]
use gpui_kit::HeadlessAppContext;
use gpui_kit::{AppContext, px, size};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(target_os = "windows")]
use gpui_kit::{App, Bounds, WindowBounds, WindowOptions, point};

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
    let selected = manifest.selected_viewport();
    let viewport = crate::layout::ViewportSnapshot {
        width: selected.width,
        height: selected.height,
        device_scale_factor: selected.device_scale_factor,
    };

    let mut captured = Vec::with_capacity(manifest.screens.len());
    for screen in &manifest.screens {
        let sink = LayoutSink::new(&screen.id, viewport);
        let view = ScreenView::build(root, screen, &manifest, sink.clone())?;
        let image = capture_image(&screen.id, view, &manifest, text_system.clone())?;

        let mut document = sink.finish();
        measure::measure_document(&image, &mut document)
            .map_err(|error| format!("could not measure {}: {error}", screen.id))?;
        if document.elements.is_empty() {
            return Err(format!(
                "{} recorded no elements; its parity markers did not paint",
                screen.id
            ));
        }
        let selected = manifest.selected_viewport();
        let expected_width = (selected.width as f32 * selected.device_scale_factor).round() as u32;
        let expected_height =
            (selected.height as f32 * selected.device_scale_factor).round() as u32;
        if image.width() != expected_width || image.height() != expected_height {
            return Err(format!(
                "{} rendered at {}x{}, expected {}x{} (viewport {}x{} at {}x)",
                screen.id,
                image.width(),
                image.height(),
                expected_width,
                expected_height,
                selected.width,
                selected.height,
                selected.device_scale_factor
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

#[cfg(not(target_os = "windows"))]
fn capture_image(
    screen_id: &str,
    view: ScreenView,
    manifest: &ScreensManifest,
    text_system: Arc<dyn gpui_kit::PlatformTextSystem>,
) -> Result<image::RgbaImage, String> {
    let mut context = HeadlessAppContext::with_platform(text_system, Arc::new(()), || {
        gpui_kit::platform::current_headless_renderer()
    });
    let window = context
        .open_window(
            size(
                px(manifest.selected_viewport().width as f32),
                px(manifest.selected_viewport().height as f32),
            ),
            |_, cx| cx.new(|_| view),
        )
        .map_err(|error| format!("could not open the {screen_id} window: {error}"))?;
    context.run_until_parked();
    context
        .capture_screenshot(window.into())
        .map_err(|error| format!("could not capture {screen_id}: {error}"))
}

#[cfg(target_os = "windows")]
fn capture_image(
    screen_id: &str,
    view: ScreenView,
    manifest: &ScreensManifest,
    _text_system: Arc<dyn gpui_kit::PlatformTextSystem>,
) -> Result<image::RgbaImage, String> {
    let selected = manifest.selected_viewport();
    let width = selected.width as f32;
    let height = selected.height as f32;
    let device_scale_factor = selected.device_scale_factor;
    let (sender, receiver) = std::sync::mpsc::channel();
    let id = screen_id.to_owned();
    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(100.0), px(100.0)),
                size: size(px(width), px(height)),
            })),
            focus: false,
            show: true,
            ..WindowOptions::default()
        };
        let result = gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| view))
            .map_err(|error| format!("could not open the {id} window: {error}"))
            .and_then(|(handle, _)| {
                handle
                    .update(cx, |_, window, cx| {
                        window.set_scale_factor(device_scale_factor);
                        window.resize(size(px(width), px(height)));
                        eprintln!(
                            "parity window: bounds={:?} viewport={:?} scale_factor={}",
                            window.bounds(),
                            window.viewport_size(),
                            window.scale_factor()
                        );
                        let _ = window.draw(cx);
                        let image = window.render_to_image()?;
                        eprintln!("parity image: {}x{}", image.width(), image.height());
                        Ok::<image::RgbaImage, gpui_kit::private::anyhow::Error>(image)
                    })
                    .map_err(|error| format!("could not capture {id}: {error}"))?
                    .map_err(|error| format!("could not capture {id}: {error}"))
                    .inspect(|_| {
                        let _ = handle.update(cx, |_, window, _| window.remove_window());
                    })
            });
        let _ = sender.send(result);
        cx.quit();
    });
    receiver
        .recv()
        .map_err(|_| format!("capture window for {screen_id} exited without a result"))?
}
