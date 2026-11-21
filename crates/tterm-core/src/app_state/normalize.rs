//! Conversion of partial UI state without changing the database format.

use super::AppState;
use serde_json::Value;

/// Keeps string group keys in their original order and accepts only timestamps
/// that the native state can represent without rounding or saturation.
pub fn normalize_app_state(state: &Value) -> AppState {
    AppState {
        last_update_check_at: state.get("lastUpdateCheckAt").and_then(exact_i64),
        collapsed_profile_group_keys: state
            .get("collapsedProfileGroupKeys")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn exact_i64(value: &Value) -> Option<i64> {
    if let Some(integer) = value.as_i64() {
        return Some(integer);
    }
    let float = value.as_f64()?;
    const TWO_POW_63: f64 = 9_223_372_036_854_775_808.0;
    (float.fract() == 0.0 && (-TWO_POW_63..TWO_POW_63).contains(&float)).then_some(float as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_missing_or_malformed_fields() {
        for value in [
            json!({}),
            Value::Null,
            json!({"lastUpdateCheckAt":"42", "collapsedProfileGroupKeys":false}),
        ] {
            assert_eq!(normalize_app_state(&value), AppState::default());
        }
    }

    #[test]
    fn accepts_only_representable_integral_timestamps() {
        for (value, expected) in [
            (json!(42.0), Some(42)),
            (json!(i64::MAX), Some(i64::MAX)),
            (json!(i64::MIN as f64), Some(i64::MIN)),
            (json!(i64::MAX as f64), None),
            (json!(42.5), None),
            (json!(-1e20), None),
            (json!(u64::MAX), None),
        ] {
            assert_eq!(
                normalize_app_state(&json!({"lastUpdateCheckAt":value})).last_update_check_at,
                expected
            );
        }
    }

    #[test]
    fn preserves_string_order_blanks_and_duplicates() {
        let state = normalize_app_state(
            &json!({"collapsedProfileGroupKeys":["gamma",1,"",null,"alpha","gamma"," "]}),
        );
        assert_eq!(
            state.collapsed_profile_group_keys,
            ["gamma", "", "alpha", "gamma", " "]
        );
    }
}
