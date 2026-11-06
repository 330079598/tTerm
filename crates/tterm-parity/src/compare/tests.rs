use super::*;
use crate::layout::{
    BoundsSnapshot, ElementSizing, PaddingSnapshot, StyleSnapshot, ViewportSnapshot,
};
use crate::manifest::ScreenSpec;
use image::{Rgba, RgbaImage};

fn style() -> StyleSnapshot {
    StyleSnapshot {
        background_color: [0, 112, 204, 255],
        border_color: [61, 61, 61, 255],
        border_width: 0.0,
        border_radius: 4.0,
        font_family: "system-ui".to_owned(),
        font_size: 14.0,
        font_weight: 500,
        line_height: 16.0,
        color: [255, 255, 255, 255],
        padding: PaddingSnapshot::symmetric(8.0, 16.0),
    }
}

fn screen() -> ScreenSpec {
    ScreenSpec {
        id: "example.button-input.default".to_owned(),
        title: "Example".to_owned(),
        fixture: "example.button-input.default.json".to_owned(),
    }
}

fn document(style: StyleSnapshot) -> LayoutDocument {
    LayoutDocument {
        version: 1,
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
    RgbaImage::from_pixel(32, 32, Rgba([27, 29, 35, 255]))
}

fn side(image: RgbaImage, document: LayoutDocument) -> Side {
    Side {
        image,
        layout: document,
    }
}

#[test]
fn identical_sides_pass() {
    let layout = document(style());
    let (report, heatmap) = compare_screen(
        &screen(),
        &side(solid(), layout.clone()),
        &side(solid(), layout),
        "abc",
    );
    assert!(report.passed, "failures: {:?}", report.failures);
    assert_eq!(report.ssim.full, 1.0);
    assert_eq!(report.ssim.masked, 1.0);
    assert_eq!(report.colors.mean_delta_e, 0.0);
    assert!(heatmap.is_some());
    assert_eq!(
        report.images.reference,
        "../reference/abc/example.button-input.default.png"
    );
}

#[test]
fn a_padding_change_fails_and_names_the_element() {
    let reference_layout = document(style());
    let mut changed = style();
    changed.padding.left = 17.0;
    let (report, _) = compare_screen(
        &screen(),
        &side(solid(), reference_layout),
        &side(solid(), document(changed)),
        "abc",
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
    let (report, _) = compare_screen(
        &screen(),
        &side(solid(), layout.clone()),
        &side(actual, layout),
        "abc",
    );
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
fn a_mismatched_image_size_is_reported_instead_of_panicking() {
    let layout = document(style());
    let (report, heatmap) = compare_screen(
        &screen(),
        &side(solid(), layout.clone()),
        &side(RgbaImage::new(16, 16), layout),
        "abc",
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
    let first = compare_screen(
        &screen(),
        &side(solid(), layout.clone()),
        &side(solid(), layout.clone()),
        "abc",
    )
    .0;
    let second = compare_screen(
        &screen(),
        &side(solid(), layout),
        &side(solid(), document(style())),
        "abc",
    )
    .0;
    let first_json = serde_json::to_string(&first).expect("report serialises");
    let second_json = serde_json::to_string(&second).expect("report serialises");
    assert_eq!(first_json, second_json);
}

#[test]
fn the_html_report_is_deterministic_and_names_the_result() {
    let layout = document(style());
    let screen_report = compare_screen(
        &screen(),
        &side(solid(), layout.clone()),
        &side(solid(), layout),
        "abc",
    )
    .0;
    let report = Report {
        schema_version: 1,
        reference: ReferenceInfo {
            frontend_commit: "abc".to_owned(),
        },
        thresholds: Thresholds {
            fixed_px: layout_check::FIXED_TOLERANCE,
            text_px: layout_check::TEXT_TOLERANCE,
            text_fraction: layout_check::TEXT_TOLERANCE_FRACTION,
            masked_ssim: MASKED_SSIM_MIN,
            full_ssim: FULL_SSIM_MIN,
        },
        screens: vec![screen_report],
        passed: true,
    };
    let first = report::render(&report);
    let second = report::render(&report);
    assert_eq!(first, second);
    assert!(first.contains("Overall: PASS"));
    assert!(first.contains("example.button-input.default"));
    assert!(!first.contains("generated"));
}
