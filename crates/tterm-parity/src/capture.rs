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
use gpui_kit::AppContext;
#[cfg(not(target_os = "windows"))]
use gpui_kit::HeadlessAppContext;
#[cfg(not(target_os = "windows"))]
use gpui_kit::{px, size};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn initialize_gpui(cx: &mut gpui_kit::App) {
    gpui_kit::init(cx);
    tterm_ui::init(cx);
}

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
    #[cfg(target_os = "windows")]
    return capture_all_windows(root, output, &manifest, text_system);

    #[cfg(not(target_os = "windows"))]
    let selected = manifest.selected_viewport();
    #[cfg(not(target_os = "windows"))]
    let viewport = crate::layout::ViewportSnapshot {
        width: selected.width,
        height: selected.height,
        device_scale_factor: selected.device_scale_factor,
    };
    #[cfg(not(target_os = "windows"))]
    let mut captured = Vec::with_capacity(manifest.screens.len());
    #[cfg(not(target_os = "windows"))]
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
    #[cfg(not(target_os = "windows"))]
    Ok(captured)
}

#[cfg(target_os = "windows")]
fn capture_all_windows(
    root: &Path,
    output: &Path,
    manifest: &ScreensManifest,
    _text_system: Arc<dyn gpui_kit::PlatformTextSystem>,
) -> Result<Vec<CapturedScreen>, String> {
    let selected = manifest.selected_viewport();
    let width = selected.width as f32;
    let height = selected.height as f32;
    let scale = selected.device_scale_factor;
    let mut pending = Vec::with_capacity(manifest.screens.len());
    for screen in &manifest.screens {
        let sink = LayoutSink::new(
            &screen.id,
            crate::layout::ViewportSnapshot {
                width: selected.width,
                height: selected.height,
                device_scale_factor: scale,
            },
        );
        let view = ScreenView::build(root, screen, manifest, sink.clone())?;
        pending.push((screen.id.clone(), sink, view));
    }

    let (sender, receiver) = std::sync::mpsc::channel();
    gpui_kit::application().run(move |cx: &mut gpui_kit::App| {
        initialize_gpui(cx);
        let mut results = Vec::with_capacity(pending.len());
        for (id, sink, view) in pending {
            let options = gpui_kit::WindowOptions {
                window_bounds: Some(gpui_kit::WindowBounds::Windowed(gpui_kit::Bounds {
                    origin: gpui_kit::point(gpui_kit::px(100.0), gpui_kit::px(100.0)),
                    size: gpui_kit::size(gpui_kit::px(width), gpui_kit::px(height)),
                })),
                focus: false,
                show: true,
                ..gpui_kit::WindowOptions::default()
            };
            let result = gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| view))
                .map_err(|error| format!("could not open the {id} window: {error}"))
                .and_then(|(handle, _)| {
                    handle
                        .update(cx, |_, window, cx| {
                            window.set_scale_factor(scale);
                            window
                                .resize(gpui_kit::size(gpui_kit::px(width), gpui_kit::px(height)));
                            let _ = window.draw(cx);
                            let image = window.render_to_image()?;
                            Ok::<image::RgbaImage, gpui_kit::private::anyhow::Error>(image)
                        })
                        .map_err(|error| format!("could not capture {id}: {error}"))?
                        .map_err(|error| format!("could not capture {id}: {error}"))
                        .inspect(|_| {
                            let _ = handle.update(cx, |_, window, _| window.remove_window());
                        })
                });
            results.push((id, sink, result));
        }
        let _ = sender.send(results);
        cx.quit();
    });

    let results = receiver
        .recv()
        .map_err(|_| "capture windows exited without a result".to_owned())?;
    let mut captured = Vec::with_capacity(results.len());
    for (id, sink, image) in results {
        let image = image?;
        let mut document = sink.finish();
        measure::measure_document(&image, &mut document)
            .map_err(|error| format!("could not measure {id}: {error}"))?;
        if document.elements.is_empty() {
            return Err(format!(
                "{id} recorded no elements; its parity markers did not paint"
            ));
        }
        let image_path = output.join(format!("{id}.png"));
        let layout_path = output.join(format!("{id}.layout.json"));
        image
            .save(&image_path)
            .map_err(|error| format!("could not write {}: {error}", image_path.display()))?;
        document.write(&layout_path)?;
        captured.push(CapturedScreen {
            id,
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
    context.update(initialize_gpui);
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
