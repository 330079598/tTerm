use super::*;
use crate::layout::{
    BoundsSnapshot, ElementSizing, PaddingSnapshot, StyleSnapshot, ViewportSnapshot,
};
use crate::manifest::ScreenSpec;
use image::{Rgba, RgbaImage};

const BACKGROUND: [u8; 4] = [0, 112, 204, 255];

fn style() -> StyleSnapshot {
    StyleSnapshot {
        background_color: BACKGROUND,
        border_color: [61, 61, 61, 255],
        border_width: 0.0,
        border_radius: 4.0,
        ring_spread: 0.0,
        font_family: "system-ui".to_owned(),
        font_size: 14.0,
        font_weight: 500,
        line_height: 16.0,
        color: [255, 255, 255, 255],
        padding: PaddingSnapshot::symmetric(8.0, 16.0),
        declared_background_color: None,
        opacity: 1.0,
        declared_border_color: None,
    }
}

fn screen() -> ScreenSpec {
    ScreenSpec {
        id: "example.button-input.default".to_owned(),
        title: "Example".to_owned(),
        fixture: "example.button-input.default.json".to_owned(),
        exceptions: Vec::new(),
    }
}

fn document(style: StyleSnapshot) -> LayoutDocument {
    LayoutDocument {
        version: crate::layout::SCHEMA_VERSION,
        screen: screen().id,
        viewport: ViewportSnapshot {
            width: 32,
            height: 32,
            device_scale_factor: 1.0,
        },
        elements: vec![crate::layout::ElementSnapshot {
            id: "example.button".to_owned(),
            sizing: ElementSizing::Text,
            bounds: BoundsSnapshot::new(4.0, 4.0, 10.0, 10.0),
            style,
        }],
        text_regions: Vec::new(),
    }
}

fn solid() -> RgbaImage {
    RgbaImage::from_pixel(32, 32, Rgba(BACKGROUND))
}

fn side(image: RgbaImage, document: LayoutDocument) -> Side {
    Side {
        image,
        layout: document,
    }
}

fn compare_identical(reference: Side, actual: Side) -> (ScreenReport, Option<RgbaImage>) {
    compare_screen(&screen(), &reference, &actual, "abc")
}

#[test]
fn identical_sides_pass() {
    let layout = document(style());
    let (report, heatmap) = compare_identical(side(solid(), layout.clone()), side(solid(), layout));
    assert!(report.passed, "failures: {:?}", report.failures);
    assert_eq!(report.ssim.full, 1.0);
    assert_eq!(report.ssim.masked, 1.0);
    assert_eq!(report.colors.mean_delta_e, 0.0);
    assert_eq!(report.screen_colors.pixels_above_one, 0);
    assert!(heatmap.is_some());
    assert_eq!(
        report.images.reference,
        "../reference/abc/example.button-input.default.png"
    );
}

#[test]
fn disabled_declarations_are_composited_once_and_still_detect_wrong_paint() {
    let mut disabled = style();
    disabled.opacity = 0.5;
    disabled.declared_background_color = Some(BACKGROUND);
    disabled.background_color = [14, 71, 120, 255];
    let image = RgbaImage::from_pixel(32, 32, Rgba(disabled.background_color));
    let snapshot = document(disabled);
    let mut failures = Vec::new();
    validate_measurements(
        &side(image.clone(), snapshot.clone()),
        "test",
        [27, 29, 35, 255],
        &mut failures,
    );
    assert!(failures.is_empty(), "{failures:?}");
    let mut wrong = snapshot;
    wrong.elements[0].style.declared_background_color = Some([255, 0, 0, 255]);
    validate_measurements(
        &side(image, wrong),
        "test",
        [27, 29, 35, 255],
        &mut failures,
    );
    assert!(
        failures
            .iter()
            .any(|failure| failure.contains("declares background"))
    );
}

#[test]
fn a_padding_change_fails_and_names_the_element() {
    let reference_layout = document(style());
    let mut changed = style();
    changed.padding.left = 17.0;
    let (report, _) = compare_identical(
        side(solid(), reference_layout),
        side(solid(), document(changed)),
    );
    assert!(!report.passed);
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.contains("example.button") && failure.contains("padding left"))
    );
}

#[test]
fn a_changed_image_fails_the_ssim_gate() {
    let layout = document(style());
    let mut actual = solid();
    for y in 0..16 {
        for x in 0..16 {
            actual.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    let (report, _) = compare_identical(side(solid(), layout.clone()), side(actual, layout));
    assert!(!report.passed);
    assert!(report.ssim.full < FULL_SSIM_MIN);
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.contains("full SSIM"))
    );
}

#[test]
fn a_declared_colour_that_does_not_match_the_pixels_fails_and_names_the_element() {
    let reference = document(style());

    // This is the evaluator's probe after the capture measured the screenshot:
    // the JSON records the red it measured, while the style snapshot still says
    // blue because the view only changed its `.bg(...)`.
    let mut probed_style = style();
    probed_style.background_color = [150, 60, 80, 255];
    probed_style.declared_background_color = Some(BACKGROUND);
    let (report, _) = compare_identical(
        side(solid(), reference),
        side(
            RgbaImage::from_pixel(32, 32, Rgba([150, 60, 80, 255])),
            document(probed_style),
        ),
    );
    assert!(!report.passed);
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.contains("example.button")
                && failure.contains("declares background")
                && failure.contains("style snapshot and the drawing disagree")),
        "failures: {:?}",
        report.failures
    );
}

#[test]
fn a_layout_colour_that_does_not_match_the_pixels_fails() {
    let reference = document(style());
    let mut stale_style = style();
    stale_style.background_color = [1, 2, 3, 255];
    let (report, _) = compare_identical(
        side(solid(), reference),
        side(solid(), document(stale_style)),
    );
    assert!(!report.passed);
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.contains("layout background"))
    );
}

#[test]
fn the_screen_colour_gate_sees_a_change_outside_text_and_edges() {
    let layout = document(style());
    let mut actual = solid();
    actual.put_pixel(20, 20, Rgba([204, 0, 0, 255]));
    let (report, _) = compare_identical(side(solid(), layout.clone()), side(actual, layout));
    assert!(!report.passed);
    assert_eq!(report.screen_colors.pixels_above_one, 1);
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.contains("screen colour gate"))
    );
}

#[test]
fn a_mismatched_image_size_is_reported_instead_of_panicking() {
    let layout = document(style());
    let (report, heatmap) = compare_identical(
        side(solid(), layout.clone()),
        side(RgbaImage::new(16, 16), layout),
    );
    assert!(!report.passed);
    assert!(heatmap.is_none());
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.contains("image size"))
    );
}

#[test]
fn the_screen_report_serialises_identically_twice() {
    let layout = document(style());
    let first = compare_identical(side(solid(), layout.clone()), side(solid(), layout.clone())).0;
    let second = compare_identical(side(solid(), layout), side(solid(), document(style()))).0;
    let first_json = serde_json::to_string(&first).expect("report serialises");
    let second_json = serde_json::to_string(&second).expect("report serialises");
    assert_eq!(first_json, second_json);
}

#[test]
fn the_html_report_is_deterministic_and_names_the_result() {
    let layout = document(style());
    let screen_report = compare_identical(side(solid(), layout.clone()), side(solid(), layout)).0;
    let report = Report {
        schema_version: 2,
        reference: ReferenceInfo {
            frontend_commit: "abc".to_owned(),
            dirty: false,
        },
        thresholds: Thresholds {
            fixed_px: layout_check::FIXED_TOLERANCE,
            text_px: layout_check::TEXT_TOLERANCE,
            text_fraction: layout_check::TEXT_TOLERANCE_FRACTION,
            masked_ssim: MASKED_SSIM_MIN,
            full_ssim: FULL_SSIM_MIN,
            screen_color_pixels: SCREEN_COLOR_MAX_PIXELS,
        },
        screens: vec![screen_report],
        passed: true,
    };
    let first = report::render(&report);
    let second = report::render(&report);
    assert_eq!(first, second);
    assert!(first.contains("Overall: PASS"));
    assert!(first.contains("example.button-input.default"));
    assert!(first.contains("screen-gate pixels"));
    assert!(!first.contains("generated"));
}
