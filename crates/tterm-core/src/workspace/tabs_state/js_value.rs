//! ECMAScript coercions used by saved, potentially malformed tab fields.

use crate::workspace::pane_templates::js_trim;
use serde_json::Value;

/// Tests JSON-backed values using ECMAScript truthiness.
pub(crate) fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => value
            .as_f64()
            .is_some_and(|value| value != 0.0 && !value.is_nan()),
        Some(Value::String(value)) => !value.is_empty(),
        _ => true,
    }
}

fn primitive(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::String(
            values
                .iter()
                .map(|value| match value {
                    Value::Null => String::new(),
                    other => primitive_string(other),
                })
                .collect::<Vec<_>>()
                .join(","),
        ),
        Value::Object(_) => Value::String("[object Object]".into()),
        other => other.clone(),
    }
}

fn primitive_string(value: &Value) -> String {
    match primitive(value) {
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => {
            crate::workspace::js_number::format_f64(value.as_f64().unwrap_or(f64::NAN))
        }
        Value::String(value) => value,
        _ => String::new(),
    }
}

fn number(value: &Value) -> f64 {
    match primitive(value) {
        Value::Null => 0.0,
        Value::Bool(value) => f64::from(u8::from(value)),
        Value::Number(value) => value.as_f64().unwrap_or(f64::NAN),
        Value::String(value) => string_number(&value),
        _ => f64::NAN,
    }
}

fn string_number(value: &str) -> f64 {
    let value = js_trim(value);
    if value.is_empty() {
        return 0.0;
    }
    if matches!(value.as_str(), "Infinity" | "+Infinity") {
        return f64::INFINITY;
    }
    if value == "-Infinity" {
        return f64::NEG_INFINITY;
    }
    for (prefix, radix) in [
        ("0x", 16),
        ("0X", 16),
        ("0b", 2),
        ("0B", 2),
        ("0o", 8),
        ("0O", 8),
    ] {
        if let Some(digits) = value.strip_prefix(prefix) {
            if digits.is_empty() {
                return f64::NAN;
            }
            return digits
                .chars()
                .try_fold(0.0, |total, digit| {
                    digit
                        .to_digit(radix)
                        .map(|digit| total * f64::from(radix) + f64::from(digit))
                })
                .unwrap_or(f64::NAN);
        }
    }
    if value.chars().any(|character| {
        !character.is_ascii_digit() && !matches!(character, '+' | '-' | '.' | 'e' | 'E')
    }) {
        return f64::NAN;
    }
    value.parse().unwrap_or(f64::NAN)
}

pub(super) fn increment(value: Option<&Value>) -> Value {
    let value = primitive(
        value
            .filter(|value| !value.is_null())
            .unwrap_or(&Value::Number(0.into())),
    );
    if let Value::String(value) = value {
        Value::String(format!("{value}1"))
    } else {
        serde_json::Number::from_f64(number(&value) + 1.0).map_or(Value::Null, Value::Number)
    }
}

pub(super) fn next_nonce(value: Option<&Value>) -> u32 {
    let value = number(&increment(value));
    if !value.is_finite() || value == 0.0 {
        0
    } else {
        value.trunc().rem_euclid(4_294_967_296.0) as u32
    }
}
