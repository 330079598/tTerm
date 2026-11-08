//! Element-by-element layout comparison with the section 5.2 thresholds.
//!
//! Fixed-size elements may move by one logical pixel; text-sized elements by
//! two, or two percent of their width, whichever is larger. Style values are
//! compared exactly (with a 0.01 px float tolerance), colours as 8-bit sRGB.
//! A colour with alpha is composited over the screen background first and then
//! compared with CIEDE2000, as the threshold table allows.

use super::color::delta_e_2000;
use super::color::srgb_to_lab;
use super::{ElementReport, LayoutReport, round_to};
use crate::layout::{BoundsSnapshot, ElementSnapshot, LayoutDocument, StyleSnapshot};
use std::collections::{BTreeMap, BTreeSet};

/// Allowed movement of a fixed-size element, in logical pixels.
pub const FIXED_TOLERANCE: f32 = 1.0;
/// Allowed movement of a text-sized element, in logical pixels.
pub const TEXT_TOLERANCE: f32 = 2.0;
/// The share of the width a text-sized element may also move by.
pub const TEXT_TOLERANCE_FRACTION: f32 = 0.02;
/// Float tolerance for style values that arrive from CSS as fractions.
const STYLE_TOLERANCE: f32 = 0.01;
/// The largest CIEDE2000 difference accepted for a composited colour.
const COLOR_TOLERANCE: f64 = 1.0;

/// Compares every marked element and returns the screen's layout result.
pub fn compare(
    reference: &LayoutDocument,
    actual: &LayoutDocument,
    backdrop: [u8; 4],
) -> LayoutReport {
    let reference_elements: BTreeMap<&str, &ElementSnapshot> = reference
        .elements
        .iter()
        .map(|element| (element.id.as_str(), element))
        .collect();
    let actual_elements: BTreeMap<&str, &ElementSnapshot> = actual
        .elements
        .iter()
        .map(|element| (element.id.as_str(), element))
        .collect();

    let mut ids: BTreeSet<&str> = BTreeSet::new();
    ids.extend(reference_elements.keys().copied());
    ids.extend(actual_elements.keys().copied());

    let mut elements = Vec::new();
    let mut failures = Vec::new();
    for id in ids {
        match (reference_elements.get(id), actual_elements.get(id)) {
            (Some(reference), Some(actual)) => {
                let report = compare_element(reference, actual, backdrop);
                failures.extend(report.failures.iter().cloned());
                elements.push(report);
            }
            (None, Some(_)) => {
                let failure = format!("{id}: present in GPUI but missing from the reference");
                failures.push(failure.clone());
                elements.push(ElementReport {
                    id: id.to_owned(),
                    passed: false,
                    max_bounds_delta: 0.0,
                    failures: vec![failure],
                });
            }
            (Some(_), None) => {
                let failure = format!("{id}: present in the reference but missing from GPUI");
                failures.push(failure.clone());
                elements.push(ElementReport {
                    id: id.to_owned(),
                    passed: false,
                    max_bounds_delta: 0.0,
                    failures: vec![failure],
                });
            }
            (None, None) => {}
        }
    }

    LayoutReport {
        passed: failures.is_empty(),
        elements,
        failures,
    }
}

fn compare_element(
    reference: &ElementSnapshot,
    actual: &ElementSnapshot,
    backdrop: [u8; 4],
) -> ElementReport {
    let id = reference.id.as_str();
    let mut failures = Vec::new();
    if reference.sizing != actual.sizing {
        failures.push(format!(
            "{id}: sizing changed from {:?} to {:?}",
            reference.sizing, actual.sizing
        ));
    }

    let allowed = match reference.sizing {
        crate::layout::ElementSizing::Fixed => FIXED_TOLERANCE,
        crate::layout::ElementSizing::Text => {
            TEXT_TOLERANCE.max(TEXT_TOLERANCE_FRACTION * reference.bounds.width)
        }
    };
    let mut max_delta = 0.0_f32;
    for (field, first, second) in bounds_fields(reference.bounds, actual.bounds) {
        let delta = (first - second).abs();
        max_delta = max_delta.max(delta);
        if delta > allowed {
            failures.push(format!(
                "{id}: {field} moved by {delta:.2} px, allowed {allowed:.2}"
            ));
        }
    }

    compare_style(id, &reference.style, &actual.style, backdrop, &mut failures);
    ElementReport {
        id: id.to_owned(),
        passed: failures.is_empty(),
        max_bounds_delta: round_to(f64::from(max_delta), 2) as f32,
        failures,
    }
}

fn bounds_fields(
    reference: BoundsSnapshot,
    actual: BoundsSnapshot,
) -> [(&'static str, f32, f32); 4] {
    [
        ("x", reference.x, actual.x),
        ("y", reference.y, actual.y),
        ("width", reference.width, actual.width),
        ("height", reference.height, actual.height),
    ]
}

fn compare_style(
    id: &str,
    reference: &StyleSnapshot,
    actual: &StyleSnapshot,
    backdrop: [u8; 4],
    failures: &mut Vec<String>,
) {
    let reference_font = canonical_font_family(&reference.font_family);
    let actual_font = canonical_font_family(&actual.font_family);
    if reference_font != actual_font {
        failures.push(format!(
            "{id}: font family {:?} -> {:?}",
            reference_font, actual_font
        ));
    }
    check_number(
        id,
        "font size",
        reference.font_size,
        actual.font_size,
        failures,
    );
    check_number(
        id,
        "font weight",
        f32::from(reference.font_weight),
        f32::from(actual.font_weight),
        failures,
    );
    check_number(
        id,
        "line height",
        reference.line_height,
        actual.line_height,
        failures,
    );
    check_number(
        id,
        "border radius",
        reference.border_radius,
        actual.border_radius,
        failures,
    );
    check_number(
        id,
        "border width",
        reference.border_width,
        actual.border_width,
        failures,
    );
    check_number(
        id,
        "padding top",
        reference.padding.top,
        actual.padding.top,
        failures,
    );
    check_number(
        id,
        "padding right",
        reference.padding.right,
        actual.padding.right,
        failures,
    );
    check_number(
        id,
        "padding bottom",
        reference.padding.bottom,
        actual.padding.bottom,
        failures,
    );
    check_number(
        id,
        "padding left",
        reference.padding.left,
        actual.padding.left,
        failures,
    );

    compare_color(
        id,
        "background",
        reference.background_color,
        actual.background_color,
        backdrop,
        failures,
    );
    if reference.border_width > 0.0 || actual.border_width > 0.0 {
        compare_color(
            id,
            "border",
            reference.border_color,
            actual.border_color,
            backdrop,
            failures,
        );
    }
    compare_color(
        id,
        "text",
        reference.color,
        actual.color,
        backdrop,
        failures,
    );
}

fn canonical_font_family(family: &str) -> String {
    let first = family
        .split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches(['"', '\''])
        .to_ascii_lowercase();
    if matches!(
        first.as_str(),
        "-apple-system"
            | "blinkmacsystemfont"
            | ".systemuifont"
            | ".applesystemuifont"
            | "segoe ui"
    ) {
        "system-ui".to_owned()
    } else {
        first
    }
}

fn check_number(id: &str, field: &str, reference: f32, actual: f32, failures: &mut Vec<String>) {
    let delta = (reference - actual).abs();
    if delta > STYLE_TOLERANCE {
        failures.push(format!(
            "{id}: {field} {reference} -> {actual} (difference {delta:.4}, allowed {STYLE_TOLERANCE})"
        ));
    }
}

fn compare_color(
    id: &str,
    field: &str,
    reference: [u8; 4],
    actual: [u8; 4],
    backdrop: [u8; 4],
    failures: &mut Vec<String>,
) {
    if reference == actual {
        return;
    }
    if reference[3] == 255 && actual[3] == 255 {
        failures.push(format!(
            "{id}: {field} colour {} -> {}",
            hex(reference),
            hex(actual)
        ));
        return;
    }
    let first = composite(reference, backdrop);
    let second = composite(actual, backdrop);
    let difference = delta_e_2000(srgb_to_lab(first), srgb_to_lab(second));
    if difference > COLOR_TOLERANCE {
        failures.push(format!(
            "{id}: {field} colour {} over the screen background became {} (CIEDE2000 {difference:.3} > {COLOR_TOLERANCE})",
            hex(reference),
            hex(second)
        ));
    }
}

fn composite(color: [u8; 4], backdrop: [u8; 4]) -> [u8; 4] {
    if color[3] == 255 {
        return color;
    }
    if color[3] == 0 {
        return [backdrop[0], backdrop[1], backdrop[2], 255];
    }
    let alpha = f64::from(color[3]) / 255.0;
    let blend = |foreground: u8, background: u8| {
        (f64::from(foreground) * alpha + f64::from(background) * (1.0 - alpha)).round() as u8
    };
    [
        blend(color[0], backdrop[0]),
        blend(color[1], backdrop[1]),
        blend(color[2], backdrop[2]),
        255,
    ]
}

fn hex(color: [u8; 4]) -> String {
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        color[0], color[1], color[2], color[3]
    )
}

#[cfg(test)]
mod tests;
