//! JavaScript-compatible string and number helpers used by the theme code.

/// Trims the characters JavaScript's `String.prototype.trim` removes.
///
/// Rust's `trim` also removes some characters JavaScript keeps, and misses
/// U+FEFF, so the set is written out explicitly.
pub fn js_trim(value: &str) -> &str {
    value.trim_matches(|character: char| {
        matches!(character,
            '\u{0009}'..='\u{000D}'
                | '\u{0020}'
                | '\u{00A0}'
                | '\u{1680}'
                | '\u{2000}'..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}')
    })
}

/// Returns what `value.slice(start)` would return for a non-negative `start`.
///
/// JavaScript slices strings by UTF-16 code unit. Rust strings are UTF-8 and
/// cannot hold an unpaired surrogate, so a slice that starts in the middle of
/// a surrogate pair replaces the trailing half with U+FFFD. That only happens
/// when `start` falls inside an astral character; every catalog id starts with
/// the eight ASCII units of `catalog:` before any such character.
pub fn js_slice_from(value: &str, start: usize) -> String {
    let mut units = 0;
    let mut result = String::new();
    for character in value.chars() {
        let length = character.len_utf16();
        if units + length <= start {
            units += length;
            continue;
        }
        if units < start {
            result.push('\u{FFFD}');
            units += length;
            continue;
        }
        result.push(character);
        units += length;
    }
    result
}

/// Rounds a number the way JavaScript's `Math.round` does.
pub fn js_round(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let floor = value.floor();
    if value - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    }
}

/// Formats a number the way JavaScript's `String(number)` does.
///
/// The theme code puts these strings into persisted CSS tokens, so `1e-7`
/// must not become `0.0000001`.
pub fn js_number_to_string(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value == 0.0 {
        return "0".into();
    }
    if value.is_infinite() {
        return if value.is_sign_positive() {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if value < 0.0 {
        return format!("-{}", js_number_to_string(-value));
    }

    let scientific = format!("{value:e}");
    let Some((mantissa, exponent)) = scientific.split_once('e') else {
        return scientific;
    };
    let digits: String = mantissa
        .chars()
        .filter(|character| *character != '.')
        .collect();
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let length = digits.len() as i32;
    let point = exponent + 1;

    if length <= point && point <= 21 {
        let mut result = digits;
        result.push_str(&"0".repeat((point - length) as usize));
        return result;
    }
    if 0 < point && point <= 21 {
        let split = point as usize;
        return format!("{}.{}", &digits[..split], &digits[split..]);
    }
    if -6 < point && point <= 0 {
        return format!("0.{}{}", "0".repeat((-point) as usize), digits);
    }

    let mut result = String::new();
    result.push_str(&digits[..1]);
    if digits.len() > 1 {
        result.push('.');
        result.push_str(&digits[1..]);
    }
    result.push('e');
    if point > 0 {
        result.push('+');
    }
    result.push_str(&(point - 1).to_string());
    result
}

/// Converts a string with JavaScript's `Number(value)` semantics.
///
/// The iterm reader feeds arbitrary plist text through `Number`, so the
/// accepted forms matter: surrounding whitespace is ignored, an empty string
/// is zero, `Infinity` is recognised, hexadecimal/binary/octal literals are
/// unsigned, and anything else is `NaN`.
pub fn js_number(value: &str) -> f64 {
    let normalized = js_trim(value);
    if normalized.is_empty() {
        return 0.0;
    }
    if let Some(rest) = normalized
        .strip_prefix('+')
        .or_else(|| normalized.strip_prefix('-'))
    {
        let sign = if normalized.starts_with('-') {
            -1.0
        } else {
            1.0
        };
        if rest == "Infinity" {
            return sign * f64::INFINITY;
        }
        return if is_decimal(rest) {
            sign * rest.parse::<f64>().unwrap_or(f64::NAN)
        } else {
            f64::NAN
        };
    }
    for (prefix, radix) in [
        ("0x", 16_u32),
        ("0X", 16),
        ("0b", 2),
        ("0B", 2),
        ("0o", 8),
        ("0O", 8),
    ] {
        if let Some(digits) = normalized.strip_prefix(prefix) {
            if digits.is_empty() || !digits.chars().all(|character| character.is_digit(radix)) {
                return f64::NAN;
            }
            let mut value = 0.0;
            for digit in digits
                .chars()
                .filter_map(|character| character.to_digit(radix))
            {
                value = value * f64::from(radix) + f64::from(digit);
            }
            return value;
        }
    }
    if normalized == "Infinity" {
        return f64::INFINITY;
    }
    if is_decimal(normalized) {
        normalized.parse::<f64>().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

fn is_decimal(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;
    let mut digits = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
        digits += 1;
    }
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return false;
    }
    if index < bytes.len() && (bytes[index] == b'e' || bytes[index] == b'E') {
        index += 1;
        if index < bytes.len() && (bytes[index] == b'+' || bytes[index] == b'-') {
            index += 1;
        }
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == start {
            return false;
        }
    }
    index == bytes.len()
}
