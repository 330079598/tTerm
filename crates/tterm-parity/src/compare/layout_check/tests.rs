use super::*;
use crate::layout::{
    ElementSizing, LayoutDocument, PaddingSnapshot, StyleSnapshot, ViewportSnapshot,
};

const BACKDROP: [u8; 4] = [27, 29, 35, 255];

fn style() -> StyleSnapshot {
    StyleSnapshot {
        background_color: [0, 112, 204, 255],
        border_color: [61, 61, 61, 255],
        border_width: 0.0,
        border_radius: 4.0,
        ring_spread: 0.0,
        font_family: "system-ui".to_owned(),
        font_size: 14.0,
        font_weight: 400,
        line_height: 16.0,
        color: [255, 255, 255, 255],
        padding: PaddingSnapshot::symmetric(8.0, 16.0),
        declared_background_color: None,
        opacity: 1.0,
        declared_border_color: None,
    }
}

fn element(id: &str, sizing: ElementSizing, bounds: BoundsSnapshot) -> ElementSnapshot {
    ElementSnapshot {
        id: id.to_owned(),
        sizing,
        bounds,
        style: style(),
    }
}

fn document(elements: Vec<ElementSnapshot>) -> LayoutDocument {
    LayoutDocument {
        version: crate::layout::SCHEMA_VERSION,
        screen: "s".to_owned(),
        viewport: ViewportSnapshot {
            width: 800,
            height: 600,
            device_scale_factor: 2.0,
        },
        elements,
        text_regions: Vec::new(),
    }
}

#[test]
fn identical_layouts_pass() {
    let bounds = BoundsSnapshot::new(328.0, 64.0, 63.75, 36.0);
    let report = compare(
        &document(vec![element("example.button", ElementSizing::Text, bounds)]),
        &document(vec![element("example.button", ElementSizing::Text, bounds)]),
        BACKDROP,
    );
    assert!(report.passed);
    assert_eq!(report.elements[0].max_bounds_delta, 0.0);
}

#[test]
fn a_ring_spread_change_fails_and_names_the_element() {
    let mut reference = document(vec![element(
        "button.focus",
        ElementSizing::Fixed,
        BoundsSnapshot::new(10.0, 10.0, 100.0, 36.0),
    )]);
    reference.elements[0].style.ring_spread = 3.0;
    let mut actual = reference.clone();
    actual.elements[0].style.ring_spread = 2.0;
    let report = compare(&reference, &actual, BACKDROP);
    assert!(!report.passed);
    assert!(report.failures.iter().any(|failure| {
        failure.contains("button.focus") && failure.contains("ring spread 3 -> 2")
    }));
}

#[test]
fn a_fixed_element_may_move_by_one_pixel() {
    let reference = document(vec![element(
        "example.input",
        ElementSizing::Fixed,
        BoundsSnapshot::new(32.0, 64.0, 288.0, 36.0),
    )]);
    let actual = document(vec![element(
        "example.input",
        ElementSizing::Fixed,
        BoundsSnapshot::new(33.0, 64.0, 288.0, 36.0),
    )]);
    assert!(compare(&reference, &actual, BACKDROP).passed);
}

#[test]
fn a_fixed_element_moving_more_than_one_pixel_fails_and_names_it() {
    let reference = document(vec![element(
        "example.input",
        ElementSizing::Fixed,
        BoundsSnapshot::new(32.0, 64.0, 288.0, 36.0),
    )]);
    let actual = document(vec![element(
        "example.input",
        ElementSizing::Fixed,
        BoundsSnapshot::new(33.5, 64.0, 288.0, 36.0),
    )]);
    let report = compare(&reference, &actual, BACKDROP);
    assert!(!report.passed);
    assert!(report.elements[0].failures[0].contains("example.input"));
    assert!(report.elements[0].failures[0].contains("x moved by 1.50"));
}

#[test]
fn a_text_sized_element_gets_two_pixels_or_two_percent() {
    let reference = document(vec![element(
        "example.button",
        ElementSizing::Text,
        BoundsSnapshot::new(328.0, 64.0, 100.0, 36.0),
    )]);
    let allowed = document(vec![element(
        "example.button",
        ElementSizing::Text,
        BoundsSnapshot::new(328.0, 64.0, 102.0, 36.0),
    )]);
    assert!(compare(&reference, &allowed, BACKDROP).passed);

    let refused = document(vec![element(
        "example.button",
        ElementSizing::Text,
        BoundsSnapshot::new(328.0, 64.0, 102.5, 36.0),
    )]);
    assert!(!compare(&reference, &refused, BACKDROP).passed);
}

#[test]
fn a_one_pixel_padding_change_is_rejected() {
    let mut actual_style = style();
    actual_style.padding.left = 17.0;
    let reference = document(vec![element(
        "example.button",
        ElementSizing::Text,
        BoundsSnapshot::new(328.0, 64.0, 63.75, 36.0),
    )]);
    let actual = LayoutDocument {
        elements: vec![ElementSnapshot {
            id: "example.button".to_owned(),
            sizing: ElementSizing::Text,
            bounds: BoundsSnapshot::new(328.0, 64.0, 65.75, 36.0),
            style: actual_style,
        }],
        ..document(Vec::new())
    };
    let report = compare(&reference, &actual, BACKDROP);
    assert!(!report.passed);
    assert!(
        report.elements[0]
            .failures
            .iter()
            .any(|failure| failure.contains("example.button") && failure.contains("padding left"))
    );
}

#[test]
fn a_missing_element_fails() {
    let reference = document(vec![element(
        "example.input",
        ElementSizing::Fixed,
        BoundsSnapshot::new(0.0, 0.0, 10.0, 10.0),
    )]);
    let report = compare(&reference, &document(Vec::new()), BACKDROP);
    assert!(!report.passed);
    assert!(report.failures[0].contains("missing from GPUI"));
}

#[test]
fn a_font_family_change_fails() {
    let mut actual_style = style();
    actual_style.font_family = "Helvetica".to_owned();
    let bounds = BoundsSnapshot::new(0.0, 0.0, 10.0, 10.0);
    let reference = document(vec![element("e", ElementSizing::Fixed, bounds)]);
    let actual = LayoutDocument {
        elements: vec![ElementSnapshot {
            id: "e".to_owned(),
            sizing: ElementSizing::Fixed,
            bounds,
            style: actual_style,
        }],
        ..document(Vec::new())
    };
    let report = compare(&reference, &actual, BACKDROP);
    assert!(!report.passed);
    assert!(report.failures[0].contains("font family"));
}

#[test]
fn native_system_fonts_are_canonicalised() {
    let mut actual_style = style();
    actual_style.font_family = "Segoe UI, sans-serif".to_owned();
    let bounds = BoundsSnapshot::new(0.0, 0.0, 10.0, 10.0);
    let reference = document(vec![element("e", ElementSizing::Fixed, bounds)]);
    let actual = LayoutDocument {
        elements: vec![ElementSnapshot {
            id: "e".to_owned(),
            sizing: ElementSizing::Fixed,
            bounds,
            style: actual_style,
        }],
        ..document(Vec::new())
    };
    assert!(compare(&reference, &actual, BACKDROP).passed);
}

#[test]
fn transparent_colours_compare_after_compositing() {
    let mut transparent = style();
    transparent.background_color = [0, 0, 0, 0];
    let bounds = BoundsSnapshot::new(0.0, 0.0, 10.0, 10.0);
    let reference = LayoutDocument {
        elements: vec![ElementSnapshot {
            id: "field".to_owned(),
            sizing: ElementSizing::Fixed,
            bounds,
            style: transparent.clone(),
        }],
        ..document(Vec::new())
    };
    assert!(compare(&reference, &reference, BACKDROP).passed);

    let mut different = transparent;
    different.background_color = [0, 0, 0, 128];
    let actual = LayoutDocument {
        elements: vec![ElementSnapshot {
            id: "field".to_owned(),
            sizing: ElementSizing::Fixed,
            bounds,
            style: different,
        }],
        ..document(Vec::new())
    };
    assert!(!compare(&reference, &actual, BACKDROP).passed);
}

#[test]
fn measured_disabled_colours_are_not_composited_again() {
    let bounds = BoundsSnapshot::new(0.0, 0.0, 10.0, 10.0);
    let mut reference = document(vec![element("disabled", ElementSizing::Fixed, bounds)]);
    let mut actual = reference.clone();
    reference.elements[0].style.opacity = 0.5;
    actual.elements[0].style.opacity = 0.5;
    reference.elements[0].style.declared_background_color = Some([61, 61, 61, 255]);
    actual.elements[0].style.declared_background_color = Some([61, 61, 61, 255]);
    reference.elements[0].style.background_color = [44, 45, 48, 255];
    actual.elements[0].style.background_color = [47, 48, 50, 255];
    assert!(!compare(&reference, &actual, BACKDROP).passed);
}
