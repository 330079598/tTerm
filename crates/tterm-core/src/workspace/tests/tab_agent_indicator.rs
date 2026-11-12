use serde_json::{Value, json};

use crate::workspace::tab_agent_indicator::{
    DEFAULT_TAB_AGENT_INDICATOR, TabAgentIndicatorConfig, TabAgentWaitingStyle,
    TabAgentWorkingStyle, normalize_tab_agent_indicator,
};

#[test]
fn fills_in_the_defaults() {
    assert_eq!(
        normalize_tab_agent_indicator(&Value::Null),
        DEFAULT_TAB_AGENT_INDICATOR
    );
    assert_eq!(
        normalize_tab_agent_indicator(&json!("dots")),
        DEFAULT_TAB_AGENT_INDICATOR
    );
}

#[test]
fn keeps_known_styles_and_drops_unknown_ones() {
    let indicator = normalize_tab_agent_indicator(&json!({
        "working": "braille",
        "waiting": "spin",
    }));
    assert_eq!(indicator.working, TabAgentWorkingStyle::Braille);
    assert_eq!(indicator.waiting, TabAgentWaitingStyle::Bars);
}

#[test]
fn keeps_the_crab_for_both() {
    let indicator = normalize_tab_agent_indicator(&json!({
        "working": "crab",
        "waiting": "crab",
    }));
    assert_eq!(indicator.working, TabAgentWorkingStyle::Crab);
    assert_eq!(indicator.waiting, TabAgentWaitingStyle::Crab);
}

#[test]
fn keeps_only_rrggbb_colors_lowercased() {
    let indicator = normalize_tab_agent_indicator(&json!({
        "working_color": "#3B9AE1",
        "waiting_color": "red",
    }));
    assert_eq!(indicator.working_color, "#3b9ae1");
    assert_eq!(indicator.waiting_color, "");
}

#[test]
fn oracle_treats_an_array_or_unknown_styles_as_defaults() {
    assert_eq!(
        normalize_tab_agent_indicator(&json!([])),
        DEFAULT_TAB_AGENT_INDICATOR
    );
    assert_eq!(
        normalize_tab_agent_indicator(&json!({"working": "spin", "waiting": "spin"})),
        DEFAULT_TAB_AGENT_INDICATOR
    );
}

#[test]
fn oracle_rejects_unicode_hex_and_wrong_lengths() {
    let indicator = normalize_tab_agent_indicator(&json!({
        "working_color": "#ＡＢＣＤＥＦ",
        "waiting_color": "#1234567",
    }));
    assert_eq!(indicator.working_color, "");
    assert_eq!(indicator.waiting_color, "");
}

#[test]
fn oracle_serializes_exactly_like_the_backend_config() {
    let indicator = normalize_tab_agent_indicator(&json!({
        "working": "braille",
        "waiting_color": "#FF0000",
    }));
    assert_eq!(
        serde_json::to_string(&indicator).unwrap(),
        r##"{"working":"braille","waiting":"bars","working_color":"","waiting_color":"#ff0000"}"##
    );
    let expected = TabAgentIndicatorConfig {
        working: TabAgentWorkingStyle::Braille,
        waiting: TabAgentWaitingStyle::Bars,
        working_color: String::new(),
        waiting_color: "#ff0000".to_owned(),
    };
    assert_eq!(indicator, expected);
}
