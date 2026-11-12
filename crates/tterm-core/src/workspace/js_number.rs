//! JSON number formatting that matches `JSON.stringify` for pane sizes.
//!
//! ECMAScript prints integral floats without a fractional part and switches
//! to exponent notation outside `[1e-6, 1e21)`. `serde_json` prints `1.0` and
//! `1e-6` differently, so persisted pane sizes use this formatter.

use serde::ser::{SerializeSeq, Serializer};
use serde_json::value::RawValue;

/// Formats a finite `f64` like `String(value)` in ECMAScript.
pub(crate) fn format_f64(value: f64) -> String {
    if value.is_nan() || value.is_infinite() {
        return "null".to_owned();
    }
    if value == 0.0 {
        return "0".to_owned();
    }
    if value < 0.0 {
        return format!("-{}", format_f64(-value));
    }

    let text = format!("{value}");
    let (integer, fraction) = match text.split_once('.') {
        Some((integer, fraction)) => (integer, fraction),
        None => (text.as_str(), ""),
    };
    let integer_length = integer.len();

    let mut digits = String::new();
    let mut decimal_position = 0i32;
    let mut first_significant = true;
    for (index, character) in integer.chars().enumerate() {
        if first_significant {
            if character == '0' {
                continue;
            }
            first_significant = false;
            decimal_position = integer_length as i32 - index as i32;
        }
        digits.push(character);
    }
    if first_significant {
        for (index, character) in fraction.chars().enumerate() {
            if character == '0' {
                continue;
            }
            decimal_position = -(index as i32);
            digits.push(character);
            digits.extend(fraction.chars().skip(index + 1));
            break;
        }
    } else if !fraction.is_empty() {
        digits.push_str(fraction);
    }
    if digits.is_empty() {
        return "0".to_owned();
    }
    while digits.ends_with('0') {
        digits.pop();
    }

    let digit_count = digits.len() as i32;
    if digit_count <= decimal_position && decimal_position <= 21 {
        let zeros = (decimal_position - digit_count).max(0) as usize;
        format!("{digits}{}", "0".repeat(zeros))
    } else if 0 < decimal_position && decimal_position <= 21 {
        let split = decimal_position as usize;
        format!("{}.{}", &digits[..split], &digits[split..])
    } else if -6 < decimal_position && decimal_position <= 0 {
        let zeros = (-decimal_position).max(0) as usize;
        format!("0.{}{}", "0".repeat(zeros), digits)
    } else {
        let exponent = decimal_position - 1;
        let sign = if exponent >= 0 { "+" } else { "-" };
        let mantissa = if digit_count == 1 {
            digits
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        format!("{mantissa}e{sign}{}", exponent.abs())
    }
}

/// Serializes a size list with ECMAScript number formatting.
pub(crate) fn serialize_sizes<S>(sizes: &[f64], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut sequence = serializer.serialize_seq(Some(sizes.len()))?;
    for value in sizes {
        let formatted = format_f64(*value);
        let raw = RawValue::from_string(formatted).map_err(serde::ser::Error::custom)?;
        sequence.serialize_element(&raw)?;
    }
    sequence.end()
}
