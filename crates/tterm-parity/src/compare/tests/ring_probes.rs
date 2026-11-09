use super::*;

const RING: [u8; 4] = [255, 255, 255, 255];
const OFFSET: [u8; 4] = [27, 29, 35, 255];

fn ring_document() -> LayoutDocument {
    let mut layout = document(style());
    layout.viewport.width = 80;
    layout.viewport.height = 80;
    layout.viewport.device_scale_factor = 2.0;
    let element = &mut layout.elements[0];
    element.id = "tabs.focus".into();
    element.bounds = BoundsSnapshot::new(24.0, 24.0, 32.0, 24.0);
    element.style.border_radius = 6.0;
    element.style.ring_spread = 4.0;
    element.style.ring_spreads = vec![2.0, 4.0];
    layout
}

// Independently rasterise the three nested rounded boxes in the CSS model.
// No antialiasing is needed to probe whether genuine fill errors survive masks.
fn ring_image(shift: f32, offset: [u8; 4]) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(160, 160, Rgba(BACKGROUND));
    for (spread, color) in [(4.0, RING), (2.0, offset), (0.0, BACKGROUND)] {
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            let local_x = (x as f32 + 0.5) / 2.0 - (40.0 + shift);
            let local_y = (y as f32 + 0.5) / 2.0 - 36.0;
            let qx = local_x.abs() - 10.0;
            let qy = local_y.abs() - 6.0;
            let distance = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - (6.0 + spread);
            if distance <= 0.0 {
                *pixel = Rgba(color);
            }
        }
    }
    image
}

#[test]
fn multilayer_ring_control_passes_without_changing_thresholds() {
    let image = ring_image(0.0, OFFSET);
    let layout = ring_document();
    let (report, _) = compare_identical(side(image.clone(), layout.clone()), side(image, layout));
    assert!(report.passed, "{:?}", report.failures);
    assert_eq!(report.screen_colors.pixels_above_one, 0);
}

#[test]
fn moving_the_painted_ring_one_logical_pixel_fails_the_screen_gate() {
    let layout = ring_document();
    let (report, _) = compare_identical(
        side(ring_image(0.0, OFFSET), layout.clone()),
        side(ring_image(1.0, OFFSET), layout),
    );
    assert!(!report.passed);
    assert!(report.screen_colors.pixels_above_one > 0);
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.contains("screen colour gate"))
    );
}

#[test]
fn changing_the_offset_fill_fails_the_screen_gate() {
    let layout = ring_document();
    let (report, _) = compare_identical(
        side(ring_image(0.0, OFFSET), layout.clone()),
        side(ring_image(0.0, [204, 0, 0, 255]), layout),
    );
    assert!(!report.passed);
    assert!(report.screen_colors.pixels_above_one > 0);
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.contains("screen colour gate"))
    );
}
