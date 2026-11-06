//! Runs the comparison and writes the machine-readable and HTML reports.
//!
//! A screen passes when every marked element is within the section 5.2 bounds
//! and style tolerances, the masked SSIM is at least 0.99 and the full-screen
//! SSIM at least 0.97. The colour-difference metric is reported for the record;
//! element colours are gated by the layout check.

pub mod color;
pub mod image_diff;
pub mod layout_check;
pub mod report;

use crate::layout::LayoutDocument;
use crate::manifest::{ScreenSpec, ScreensManifest};
use crate::reference;
use image::RgbaImage;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Masked-SSIM floor from `ui-parity.md` section 5.2.
pub const MASKED_SSIM_MIN: f64 = 0.99;
/// Full-screen SSIM floor from `ui-parity.md` section 5.2.
pub const FULL_SSIM_MIN: f64 = 0.97;

/// What a compare run produced.
#[derive(Debug)]
pub struct CompareOutcome {
    /// True when every screen passed.
    pub passed: bool,
    /// `target/parity/report/result.json`.
    pub result_path: PathBuf,
    /// `target/parity/report/index.html`.
    pub report_path: PathBuf,
    /// One report per manifest screen.
    pub screens: Vec<ScreenReport>,
}

/// One screen's result, also the JSON payload.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenReport {
    /// Screen id from the manifest.
    pub id: String,
    /// Human-readable title from the manifest.
    pub title: String,
    /// Whether this screen met every threshold.
    pub passed: bool,
    /// Element and style comparison.
    pub layout: LayoutReport,
    /// Whole-image CIEDE2000 statistics over the unmasked pixels.
    pub colors: ColorReport,
    /// Full and text-masked SSIM.
    pub ssim: SsimReport,
    /// Heatmap path relative to the report directory, empty when the images
    /// could not be compared.
    pub heatmap: String,
    /// Where the two screenshots are, relative to the report directory.
    pub images: ImagePaths,
    /// Everything that failed, flattened for the summary.
    pub failures: Vec<String>,
}

/// Element-level layout result.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutReport {
    /// Whether every element matched.
    pub passed: bool,
    /// Per-element detail, sorted by id.
    pub elements: Vec<ElementReport>,
    /// Every element failure with the element id in the message.
    pub failures: Vec<String>,
}

/// One element's layout result.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementReport {
    /// Shared element id.
    pub id: String,
    /// Whether this element matched.
    pub passed: bool,
    /// Largest absolute change of x, y, width or height.
    pub max_bounds_delta: f32,
    /// The fields that failed, with element id and values.
    pub failures: Vec<String>,
}

/// Image-level colour difference.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorReport {
    /// Mean CIEDE2000 over unmasked pixels.
    pub mean_delta_e: f64,
    /// Largest CIEDE2000 over unmasked pixels.
    pub max_delta_e: f64,
    /// Pixels whose difference is above 1.
    pub pixels_above_one: u64,
}

/// The two SSIM numbers.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SsimReport {
    /// Whole screenshot, text included.
    pub full: f64,
    /// Screenshot with every text rectangle masked out.
    pub masked: f64,
}

/// Relative image paths for the HTML report.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePaths {
    /// Reference PNG.
    pub reference: String,
    /// GPUI PNG.
    pub actual: String,
}

/// The `result.json` document.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Schema version of this report.
    pub schema_version: u32,
    /// Which reference was compared.
    pub reference: ReferenceInfo,
    /// The thresholds that were applied.
    pub thresholds: Thresholds,
    /// Per-screen results.
    pub screens: Vec<ScreenReport>,
    /// Overall result.
    pub passed: bool,
}

/// Reference identity in the report.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceInfo {
    /// Last commit that touched the reference frontend.
    pub frontend_commit: String,
}

/// The thresholds, written into the report so a reader does not need the docs.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Thresholds {
    /// Fixed element movement allowance.
    pub fixed_px: f32,
    /// Text-sized element movement allowance.
    pub text_px: f32,
    /// Text-sized width fraction allowance.
    pub text_fraction: f32,
    /// Masked SSIM floor.
    pub masked_ssim: f64,
    /// Full SSIM floor.
    pub full_ssim: f64,
}

/// Compares one reference directory with one actual directory.
pub fn run(
    root: &Path,
    reference_dir: &Path,
    actual_dir: &Path,
    report_dir: &Path,
) -> Result<CompareOutcome, String> {
    let manifest = ScreensManifest::load(root)?;
    let commit = reference::recorded_commit(reference_dir)?;
    if report_dir.exists() {
        fs::remove_dir_all(report_dir)
            .map_err(|error| format!("could not clear {}: {error}", report_dir.display()))?;
    }
    fs::create_dir_all(report_dir.join("heatmaps"))
        .map_err(|error| format!("could not create {}: {error}", report_dir.display()))?;

    let mut screens = Vec::with_capacity(manifest.screens.len());
    for screen in &manifest.screens {
        let reference_side = load_side(reference_dir, screen)?;
        let actual_side = load_side(actual_dir, screen)?;
        let (mut report, heatmap) = compare_screen(screen, &reference_side, &actual_side, &commit);
        if let Some(heatmap) = heatmap {
            let relative = format!("heatmaps/{}.png", screen.id);
            let path = report_dir.join(&relative);
            heatmap
                .save(&path)
                .map_err(|error| format!("could not write {}: {error}", path.display()))?;
            report.heatmap = relative;
        }
        screens.push(report);
    }

    let passed = screens.iter().all(|screen| screen.passed);
    let report = Report {
        schema_version: 1,
        reference: ReferenceInfo {
            frontend_commit: commit,
        },
        thresholds: Thresholds {
            fixed_px: layout_check::FIXED_TOLERANCE,
            text_px: layout_check::TEXT_TOLERANCE,
            text_fraction: layout_check::TEXT_TOLERANCE_FRACTION,
            masked_ssim: MASKED_SSIM_MIN,
            full_ssim: FULL_SSIM_MIN,
        },
        screens,
        passed,
    };

    let result_path = report_dir.join("result.json");
    let result_text = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("could not serialize the report: {error}"))?;
    fs::write(&result_path, format!("{result_text}\n"))
        .map_err(|error| format!("could not write {}: {error}", result_path.display()))?;
    let report_path = report_dir.join("index.html");
    fs::write(&report_path, report::render(&report))
        .map_err(|error| format!("could not write {}: {error}", report_path.display()))?;

    Ok(CompareOutcome {
        passed,
        result_path,
        report_path,
        screens: report.screens,
    })
}

struct Side {
    image: RgbaImage,
    layout: LayoutDocument,
}

fn load_side(directory: &Path, screen: &ScreenSpec) -> Result<Side, String> {
    let image_path = directory.join(format!("{}.png", screen.id));
    let layout_path = directory.join(format!("{}.layout.json", screen.id));
    let image = image::open(&image_path)
        .map_err(|error| format!("could not read {}: {error}", image_path.display()))?
        .to_rgba8();
    let layout = LayoutDocument::load(&layout_path)?;
    if layout.screen != screen.id {
        return Err(format!(
            "{} says it is screen \"{}\", expected \"{}\"",
            layout_path.display(),
            layout.screen,
            screen.id
        ));
    }
    Ok(Side { image, layout })
}

fn compare_screen(
    screen: &ScreenSpec,
    reference: &Side,
    actual: &Side,
    commit: &str,
) -> (ScreenReport, Option<RgbaImage>) {
    let backdrop = reference.image.get_pixel(0, 0).0;
    let layout = layout_check::compare(&reference.layout, &actual.layout, backdrop);
    let mut failures = layout.failures.clone();
    let mut heatmap = None;
    let mut ssim = SsimReport {
        full: 0.0,
        masked: 0.0,
    };
    let mut colors = ColorReport {
        mean_delta_e: 0.0,
        max_delta_e: 0.0,
        pixels_above_one: 0,
    };

    if reference.image.dimensions() != actual.image.dimensions() {
        let (reference_width, reference_height) = reference.image.dimensions();
        let (actual_width, actual_height) = actual.image.dimensions();
        failures.push(format!(
            "image size {reference_width}x{reference_height} -> {actual_width}x{actual_height}"
        ));
    } else if reference.layout.viewport != actual.layout.viewport {
        failures.push(format!(
            "viewport {:?} -> {:?}",
            reference.layout.viewport, actual.layout.viewport
        ));
    } else {
        let (width, height) = reference.image.dimensions();
        let mask = image_diff::text_mask(&reference.layout, &actual.layout, width, height);
        let full = ssim_rounded(image_diff::ssim(&reference.image, &actual.image, None));
        let masked = ssim_rounded(image_diff::ssim(
            &reference.image,
            &actual.image,
            Some(&mask),
        ));
        ssim = SsimReport { full, masked };
        if masked < MASKED_SSIM_MIN {
            failures.push(format!(
                "masked SSIM {masked:.6} is below {MASKED_SSIM_MIN}"
            ));
        }
        if full < FULL_SSIM_MIN {
            failures.push(format!("full SSIM {full:.6} is below {FULL_SSIM_MIN}"));
        }
        let difference = image_diff::color_difference(&reference.image, &actual.image, &mask);
        colors = ColorReport {
            mean_delta_e: round_to(difference.mean_delta_e, 4),
            max_delta_e: round_to(difference.max_delta_e, 4),
            pixels_above_one: difference.pixels_above_one,
        };
        heatmap = Some(image_diff::heatmap(&reference.image, &actual.image));
    }

    let passed = layout.passed && failures.is_empty();
    let report = ScreenReport {
        id: screen.id.clone(),
        title: screen.title.clone(),
        passed,
        layout,
        colors,
        ssim,
        heatmap: String::new(),
        images: ImagePaths {
            reference: format!("../reference/{commit}/{}.png", screen.id),
            actual: format!("../actual/{}.png", screen.id),
        },
        failures,
    };
    (report, heatmap)
}

fn ssim_rounded(value: f64) -> f64 {
    round_to(value, 6)
}

/// Rounds for stable JSON and stable report output across runs.
pub fn round_to(value: f64, digits: i32) -> f64 {
    let factor = 10.0_f64.powi(digits);
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests;
