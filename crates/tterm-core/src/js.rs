//! ECMAScript string behaviour the ported frontend modules rely on.
//!
//! JavaScript strings are sequences of UTF-16 code units, and its whitespace
//! set differs from Rust's, so the ports that trim, split or measure text use
//! these helpers rather than the standard library's.

/// Matches JavaScript's `\s`: everything Rust calls whitespace, plus the
/// byte-order mark.
pub(crate) fn is_js_whitespace(character: char) -> bool {
    character.is_whitespace() || character == '\u{FEFF}'
}

/// Trims the characters JavaScript's `String.prototype.trim` removes.
pub(crate) fn trim_js(value: &str) -> &str {
    value.trim_matches(is_js_whitespace)
}

/// The length JavaScript reports for `value`, in UTF-16 code units.
pub(crate) fn utf16_len(value: &str) -> usize {
    value.chars().map(char::len_utf16).sum()
}

/// Returns what `value.slice(start, end)` would return.
///
/// A range that splits a surrogate pair cannot be represented in Rust, so the
/// half it covers becomes U+FFFD, as the theme catalog port already does.
pub(crate) fn utf16_slice(value: &str, start: usize, end: usize) -> String {
    if start >= end {
        return String::new();
    }

    let mut units = 0;
    let mut result = String::new();
    for character in value.chars() {
        let length = character.len_utf16();
        let from = units;
        let to = units + length;
        units = to;
        if to <= start {
            continue;
        }
        if from >= end {
            break;
        }
        if from >= start && to <= end {
            result.push(character);
        } else {
            result.push('\u{FFFD}');
        }
    }
    result
}

/// Formats `value` like ECMAScript's `Number.prototype.toFixed`.
///
/// The rounding uses the exact binary value, so a tie such as `1.25` rounds
/// away from zero to `1.3`, as it does in JavaScript.
pub(crate) fn to_fixed(value: f64, digits: u32) -> String {
    let precision = digits as usize;
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity".to_owned()
        } else {
            "Infinity".to_owned()
        };
    }
    if value == 0.0 {
        return if digits == 0 {
            "0".to_owned()
        } else {
            format!("0.{}", "0".repeat(digits as usize))
        };
    }

    let negative = value.is_sign_negative();
    let bits = value.abs().to_bits();
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (mantissa, exponent) = if exponent_bits == 0 {
        (u128::from(fraction), -1074)
    } else {
        (
            u128::from(fraction | (1_u64 << 52)),
            exponent_bits - 1023 - 52,
        )
    };

    let mut scale = 1_u128;
    for _ in 0..digits {
        let Some(next) = scale.checked_mul(5) else {
            return format!("{value:.precision$}");
        };
        scale = next;
    }
    let Some(scaled) = mantissa.checked_mul(scale) else {
        return format!("{value:.precision$}");
    };
    let shift = exponent + digits as i32;
    let rounded = if shift >= 0 {
        let Ok(shift) = u32::try_from(shift) else {
            return format!("{value:.precision$}");
        };
        let Some(integer) = scaled.checked_shl(shift) else {
            return format!("{value:.precision$}");
        };
        integer
    } else {
        let denominator_shift = shift.unsigned_abs();
        if denominator_shift >= 128 {
            0
        } else {
            let denominator = 1_u128 << denominator_shift;
            let integer = scaled / denominator;
            let remainder = scaled % denominator;
            if remainder >= denominator.div_ceil(2) {
                integer + 1
            } else {
                integer
            }
        }
    };

    let mut text = rounded.to_string();
    if digits > 0 {
        if text.len() <= precision {
            text = format!("{}{text}", "0".repeat(precision + 1 - text.len()));
        }
        let split = text.len() - precision;
        text.insert(split, '.');
    }
    if negative { format!("-{text}") } else { text }
}

#[cfg(test)]
mod tests {
    use super::{to_fixed, trim_js, utf16_len, utf16_slice};

    #[test]
    fn trims_what_javascript_trims() {
        assert_eq!(trim_js("\u{FEFF} x \u{FEFF}"), "x");
        assert_eq!(trim_js("\u{2003}\tx\n"), "x");
    }

    #[test]
    fn measures_and_slices_utf16_code_units() {
        assert_eq!(utf16_len("a😀b"), 4);
        assert_eq!(utf16_slice("a😀b", 1, 3), "😀");
        assert_eq!(utf16_slice("a😀b", 2, 3), "\u{FFFD}");
        assert_eq!(utf16_slice("a😀b", 3, 5), "b");
        assert_eq!(utf16_slice("abc", 2, 2), "");
    }

    #[test]
    fn rounds_fixed_decimals_like_javascript() {
        assert_eq!(to_fixed(1.25, 1), "1.3");
        assert_eq!(to_fixed(-1.25, 1), "-1.3");
        assert_eq!(to_fixed(1.35, 1), "1.4");
        assert_eq!(to_fixed(1.005, 2), "1.00");
        assert_eq!(to_fixed(2.5, 1), "2.5");
        assert_eq!(to_fixed(1280.0 / 1024.0, 1), "1.3");
        assert_eq!(to_fixed(0.0, 1), "0.0");
    }
}
