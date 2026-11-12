//! The accepted color syntax mirrors ThemeEditor/colorUtils.ts::colorToHex.
use regex::Regex;

pub(crate) fn parse_color(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        let expanded = match hex.len() {
            3 => hex
                .chars()
                .flat_map(|character| [character, character])
                .collect::<String>(),
            6 => hex.to_string(),
            _ => return None,
        };
        return expanded
            .chars()
            .all(|character| character.is_ascii_hexdigit())
            .then(|| format!("#{}", expanded.to_ascii_lowercase()));
    }
    // TS deliberately accepts a function prefix without requiring its closing parenthesis.
    let rgb = Regex::new(r"(?i)^rgba?\(\s*([0-9]+(?:\.[0-9]+)?)[\s,]+([0-9]+(?:\.[0-9]+)?)[\s,]+([0-9]+(?:\.[0-9]+)?)").ok()?;
    if let Some(parts) = rgb.captures(value) {
        return Some(rgb_hex(
            parts.get(1)?.as_str().parse().ok()?,
            parts.get(2)?.as_str().parse().ok()?,
            parts.get(3)?.as_str().parse().ok()?,
        ));
    }
    let token =
        Regex::new(r"^([0-9]+(?:\.[0-9]+)?)\s+([0-9]+(?:\.[0-9]+)?)%\s+([0-9]+(?:\.[0-9]+)?)%$")
            .ok()?;
    let function = Regex::new(r"(?i)^hsla?\(\s*([0-9]+(?:\.[0-9]+)?)(?:deg)?[\s,]+([0-9]+(?:\.[0-9]+)?)%[\s,]+([0-9]+(?:\.[0-9]+)?)%").ok()?;
    let parts = token.captures(value).or_else(|| function.captures(value))?;
    let hue: f64 = parts.get(1)?.as_str().parse().ok()?;
    let saturation: f64 = parts.get(2)?.as_str().parse().ok()?;
    let lightness: f64 = parts.get(3)?.as_str().parse().ok()?;
    Some(hsl_hex(hue, saturation, lightness))
}
fn rgb_hex(red: f64, green: f64, blue: f64) -> String {
    let channel = |value: f64| value.clamp(0.0, 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(red),
        channel(green),
        channel(blue)
    )
}
fn hsl_hex(hue: f64, saturation: f64, lightness: f64) -> String {
    let hue = ((hue % 360.0 + 360.0) % 360.0) / 360.0;
    let saturation = saturation.clamp(0.0, 100.0) / 100.0;
    let lightness = lightness.clamp(0.0, 100.0) / 100.0;
    if saturation == 0.0 {
        return rgb_hex(lightness * 255.0, lightness * 255.0, lightness * 255.0);
    }
    let upper = if lightness < 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let lower = 2.0 * lightness - upper;
    let channel = |mut shifted: f64| {
        if shifted < 0.0 {
            shifted += 1.0;
        }
        if shifted > 1.0 {
            shifted -= 1.0;
        }
        let value = if shifted < 1.0 / 6.0 {
            lower + (upper - lower) * 6.0 * shifted
        } else if shifted < 0.5 {
            upper
        } else if shifted < 2.0 / 3.0 {
            lower + (upper - lower) * (2.0 / 3.0 - shifted) * 6.0
        } else {
            lower
        };
        value * 255.0
    };
    rgb_hex(
        channel(hue + 1.0 / 3.0),
        channel(hue),
        channel(hue - 1.0 / 3.0),
    )
}
