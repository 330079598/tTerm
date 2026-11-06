//! CIEDE2000 for the colour-difference metric.
//!
//! Screenshots are 8-bit sRGB. The comparison converts the two pixels to
//! CIELAB (D65) and applies the CIEDE2000 formula from Sharma, Wu and Dalal,
//! "The CIEDE2000 Color-Difference Formula" (2005), with all weights at 1.

/// Converts one opaque sRGB colour to CIELAB (D65).
pub fn srgb_to_lab(color: [u8; 4]) -> [f64; 3] {
    let red = linearize(color[0]);
    let green = linearize(color[1]);
    let blue = linearize(color[2]);

    let x = (0.412_456_4 * red + 0.357_576_1 * green + 0.180_437_5 * blue) / 0.950_47;
    let y = 0.212_672_9 * red + 0.715_152_2 * green + 0.072_175_0 * blue;
    let z = (0.019_333_9 * red + 0.119_192_0 * green + 0.950_304_1 * blue) / 1.088_83;

    let fx = lab_curve(x);
    let fy = lab_curve(y);
    let fz = lab_curve(z);
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// The CIEDE2000 difference between two CIELAB colours.
pub fn delta_e_2000(first: [f64; 3], second: [f64; 3]) -> f64 {
    let [l1, a1, b1] = first;
    let [l2, a2, b2] = second;

    let c1 = a1.hypot(b1);
    let c2 = a2.hypot(b2);
    let c_bar = (c1 + c2) / 2.0;
    let c_bar_seven = c_bar.powi(7);
    let g = 0.5 * (1.0 - (c_bar_seven / (c_bar_seven + 25.0_f64.powi(7))).sqrt());

    let a1_prime = (1.0 + g) * a1;
    let a2_prime = (1.0 + g) * a2;
    let c1_prime = a1_prime.hypot(b1);
    let c2_prime = a2_prime.hypot(b2);

    let h1_prime = hue_angle(b1, a1_prime);
    let h2_prime = hue_angle(b2, a2_prime);

    let delta_l = l2 - l1;
    let delta_c = c2_prime - c1_prime;
    let delta_h_angle = if c1_prime * c2_prime == 0.0 {
        0.0
    } else if (h2_prime - h1_prime).abs() <= 180.0 {
        h2_prime - h1_prime
    } else if h2_prime - h1_prime > 180.0 {
        h2_prime - h1_prime - 360.0
    } else {
        h2_prime - h1_prime + 360.0
    };
    let delta_h = 2.0 * (c1_prime * c2_prime).sqrt() * (delta_h_angle.to_radians() / 2.0).sin();

    let l_bar = (l1 + l2) / 2.0;
    let c_bar_prime = (c1_prime + c2_prime) / 2.0;
    let h_bar_prime = if c1_prime * c2_prime == 0.0 {
        h1_prime + h2_prime
    } else if (h1_prime - h2_prime).abs() <= 180.0 {
        (h1_prime + h2_prime) / 2.0
    } else if h1_prime + h2_prime < 360.0 {
        (h1_prime + h2_prime + 360.0) / 2.0
    } else {
        (h1_prime + h2_prime - 360.0) / 2.0
    };

    let t = 1.0 - 0.17 * (h_bar_prime - 30.0).to_radians().cos()
        + 0.24 * (2.0 * h_bar_prime).to_radians().cos()
        + 0.32 * (3.0 * h_bar_prime + 6.0).to_radians().cos()
        - 0.20 * (4.0 * h_bar_prime - 63.0).to_radians().cos();
    let delta_theta = 30.0 * (-((h_bar_prime - 275.0) / 25.0).powi(2)).exp();
    let c_bar_prime_seven = c_bar_prime.powi(7);
    let r_c = 2.0 * (c_bar_prime_seven / (c_bar_prime_seven + 25.0_f64.powi(7))).sqrt();
    let l_offset = (l_bar - 50.0).powi(2);
    let s_l = 1.0 + 0.015 * l_offset / (20.0 + l_offset).sqrt();
    let s_c = 1.0 + 0.045 * c_bar_prime;
    let s_h = 1.0 + 0.015 * c_bar_prime * t;
    let r_t = -r_c * (2.0 * delta_theta).to_radians().sin();

    let lightness = delta_l / s_l;
    let chroma = delta_c / s_c;
    let hue = delta_h / s_h;
    (lightness * lightness + chroma * chroma + hue * hue + r_t * chroma * hue).sqrt()
}

fn linearize(channel: u8) -> f64 {
    let value = f64::from(channel) / 255.0;
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn lab_curve(value: f64) -> f64 {
    if value > 0.008_856 {
        value.cbrt()
    } else {
        7.787 * value + 16.0 / 116.0
    }
}

fn hue_angle(b: f64, a: f64) -> f64 {
    if a == 0.0 && b == 0.0 {
        return 0.0;
    }
    let angle = b.atan2(a).to_degrees();
    if angle < 0.0 { angle + 360.0 } else { angle }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_colours_have_no_difference() {
        let lab = srgb_to_lab([0, 112, 204, 255]);
        assert_eq!(delta_e_2000(lab, lab), 0.0);
    }

    #[test]
    fn black_and_white_differ_by_one_hundred() {
        let difference = delta_e_2000(
            srgb_to_lab([0, 0, 0, 255]),
            srgb_to_lab([255, 255, 255, 255]),
        );
        assert!(
            (difference - 100.0).abs() < 0.01,
            "difference was {difference}"
        );
    }

    #[test]
    fn matches_the_sharma_reference_pair() {
        // Sharma, Wu and Dalal (2005), table 1, pair 1: expected 2.0425.
        let first = [50.0, 2.677_2, -79.775_1];
        let second = [50.0, 0.0, -82.748_5];
        let difference = delta_e_2000(first, second);
        assert!(
            (difference - 2.0425).abs() < 0.0001,
            "difference was {difference}"
        );
    }

    #[test]
    fn matches_the_sharma_weighted_pair() {
        // Table 1, pair 17: expected 27.1492.
        let first = [50.0, 2.5, 0.0];
        let second = [73.0, 25.0, -18.0];
        let difference = delta_e_2000(first, second);
        assert!(
            (difference - 27.149_2).abs() < 0.001,
            "difference was {difference}"
        );
    }

    #[test]
    fn the_dark_theme_blue_is_where_the_stylesheet_says() {
        let lab = srgb_to_lab([0, 112, 204, 255]);
        assert!((lab[0] - 46.903).abs() < 0.05, "L* was {}", lab[0]);
        assert!((lab[1] - 8.851).abs() < 0.05, "a* was {}", lab[1]);
        assert!((lab[2] - -54.887).abs() < 0.05, "b* was {}", lab[2]);
    }
}
