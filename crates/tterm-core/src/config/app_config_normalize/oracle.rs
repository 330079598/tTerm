use super::normalize_config;
use crate::config::AppConfig;
use crate::config::limits;
use serde_json::{Value, json};

fn platform_oracle() -> Value {
    let file = if cfg!(windows) {
        include_str!("../../../../../docs/gpui-migration/reports/P1-11c-evidence/node-windows.json")
    } else if cfg!(target_os = "macos") {
        include_str!("../../../../../docs/gpui-migration/reports/P1-11c-evidence/node-macos.json")
    } else {
        include_str!("../../../../../docs/gpui-migration/reports/P1-11c-evidence/node-linux.json")
    };
    serde_json::from_str(file).unwrap()
}

#[test]
fn settings_limits_and_sound_choices_match_typescript() {
    let oracle = platform_oracle();
    let expected = &oracle["values"]["appConfig.constants"];
    macro_rules! constants {
        ($($name:ident),+ $(,)?) => {$(
            assert_eq!(json!(limits::$name), expected[stringify!($name)], "{}", stringify!($name));
        )+};
    }
    macro_rules! ranges {
        ($($name:ident),+ $(,)?) => {$(
            let bounds = &expected[stringify!($name)];
            assert_eq!(Some(f64::from(*limits::$name.start())), bounds["min"].as_f64(), "{} min", stringify!($name));
            assert_eq!(Some(f64::from(*limits::$name.end())), bounds["max"].as_f64(), "{} max", stringify!($name));
        )+};
    }
    constants!(
        NOTIFICATION_SOUND_NAMES,
        HIDDEN_RENDERER_RELEASE_SECS_OPTIONS,
        DEFAULT_HIDDEN_RENDERER_RELEASE_SECS,
        DEFAULT_TERMINAL_FONT_SIZE,
        DEFAULT_PREDICTIVE_ECHO_THRESHOLD_MS,
        PREDICTIVE_ECHO_THRESHOLD_MIN_MS,
        PREDICTIVE_ECHO_THRESHOLD_MAX_MS,
        TERMINAL_LOG_MAX_RETENTION_DAYS,
        TERMINAL_LOG_MAX_TOTAL_MB,
        MAX_BANDWIDTH_LIMIT_KIB,
        RECONNECT_MAX_ATTEMPTS_MIN,
        RECONNECT_MAX_ATTEMPTS_MAX
    );
    ranges!(
        TERMINAL_FONT_SIZE_RANGE,
        TERMINAL_LINE_HEIGHT_RANGE,
        TERMINAL_LETTER_SPACING_RANGE,
        WINDOW_OPACITY_PERCENT_RANGE,
        WINDOW_BLUR_RADIUS_RANGE,
        NOTIFY_COMMAND_MIN_SECS_RANGE,
        TOAST_MAX_VISIBLE_RANGE
    );
    assert_eq!(
        expected["defaultRenderer"],
        limits::DEFAULT_TERMINAL_RENDERER
    );
}

fn compare_shape(actual: &Value, expected: &Value) {
    for (key, value) in expected.as_object().unwrap() {
        if value.is_number() {
            assert_eq!(actual[key].as_f64(), value.as_f64(), "field {key}");
        } else {
            assert_eq!(&actual[key], value, "field {key}");
        }
    }
    assert_eq!(
        actual.as_object().unwrap().len(),
        expected.as_object().unwrap().len()
    );
}

#[test]
fn all_default_and_normalized_fields_match_the_platform_oracle() {
    let oracle = platform_oracle();
    let mut expected = oracle["values"]["appConfig.defaultConfig"].clone();
    let config = AppConfig::default();
    expected["language"] = json!(crate::i18n::language::Language::system().code());
    expected["update_channel"] = json!(crate::config::update_channel::default_update_channel());
    if !cfg!(windows) {
        expected["secret_storage_mode"] = json!("hybrid");
    }
    compare_shape(&serde_json::to_value(config).unwrap(), &expected);
    expected["secret_storage_mode"] = json!("system");
    expected["ui_style"] = json!("classic");
    expected["update_channel"] = json!("stable");
    compare_shape(
        &serde_json::to_value(normalize_config(&json!({})).unwrap()).unwrap(),
        &expected,
    );
}

#[test]
fn replay_typescript_partial_configs() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../../docs/gpui-migration/reports/P1-11c-evidence/config-replay.json"
    ))
    .unwrap();
    for case in cases {
        let input = &case["input"];
        let result = serde_json::to_value(normalize_config(input).unwrap()).unwrap();
        for (key, expected) in case["expected"].as_object().unwrap() {
            let native = match (key.as_str(), expected) {
                ("font_size", value) if value == "14" => Some(json!(14)),
                ("scrollback_lines", value) if value == &json!(-1) => Some(json!(10000)),
                ("local_shells", value) if value == &json!([{"id":1}]) => Some(json!([])),
                ("hidden_shells", value) if value == &json!(["a", 1]) => Some(json!([])),
                ("theme", value) if value == &json!(3) => Some(json!("default")),
                _ => None,
            };
            let expected = native.as_ref().unwrap_or(expected);
            if expected.is_number() {
                assert_eq!(result[key].as_f64(), expected.as_f64(), "{input}: {key}");
            } else {
                assert_eq!(&result[key], expected, "{input}: {key}");
            }
        }
    }
}

#[test]
fn unrepresentable_spread_fields_do_not_discard_valid_siblings() {
    let config = normalize_config(&json!({"theme": false,"font_size":14.5,"scrollback_lines":7.0,"language":"zh","hidden_shells":[true]})).unwrap();
    assert_eq!(config.theme, AppConfig::default().theme);
    assert_eq!(config.font_size, AppConfig::default().font_size);
    assert_eq!(config.scrollback_lines, 7);
    assert_eq!(config.language, "zh");
    assert!(config.hidden_shells.is_empty());
}
