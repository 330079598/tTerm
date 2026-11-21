//! Values kept by the frontend object spread and structured settings with their own rules.

use crate::config::AppConfig;
use crate::config::monitor_metrics::normalize_monitor_visible_metrics;
use crate::keymap::config::normalize_keymap;
use crate::keyword_highlight::normalize_keyword_highlight_config;
use indexmap::IndexSet;
use serde_json::Value;
use tterm_terminal::js_text::js_trim;

pub(super) fn passthrough(config: &mut AppConfig, input: &Value) {
    macro_rules! copy_fields {
        ($($field:ident),+ $(,)?) => {$(
            if let Some(value) = input.get(stringify!($field))
                && let Ok(parsed) = serde_json::from_value(value.clone()) {
                config.$field = parsed;
            }
        )+};
    }
    copy_fields!(
        theme,
        language,
        font_family,
        cursor_style,
        terminal_shell,
        terminal_shell_custom_path,
        terminal_shell_custom_args,
        terminal_shell_id,
        terminal_shell_wsl_distro,
        local_shells,
        hidden_shells,
        secret_vault_enabled
    );
    if let Some(size) = unsigned(&input["font_size"]).and_then(|number| number.try_into().ok()) {
        config.font_size = size;
    }
    if let Some(lines) = unsigned(&input["scrollback_lines"]) {
        config.scrollback_lines = lines;
    }
}

fn unsigned(value: &Value) -> Option<u32> {
    let number = value.as_f64()?;
    (number.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&number))
        .then_some(number as u32)
}

fn strings(value: &Value) -> impl Iterator<Item = &str> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
}

pub(super) fn collections(config: &mut AppConfig, input: &Value) -> serde_json::Result<()> {
    config.favorite_themes = strings(&input["favorite_themes"])
        .filter(|id| !id.is_empty())
        .collect::<IndexSet<_>>()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let metrics: Vec<_> = strings(&input["monitor_visible_metrics"])
        .map(str::to_owned)
        .collect();
    config.monitor_visible_metrics = normalize_monitor_visible_metrics(Some(&metrics))
        .into_iter()
        .map(|id| id.as_str().to_owned())
        .collect();
    config.sudo_prompt_patterns = strings(&input["sudo_prompt_patterns"])
        .filter(|pattern| !js_trim(pattern).is_empty())
        .map(str::to_owned)
        .collect();
    config.agent_config_dirs = input["agent_config_dirs"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(key, value)| {
            Some((
                key.clone(),
                value.as_str().filter(|text| !text.is_empty())?.to_owned(),
            ))
        })
        .collect();
    config.keymap.bindings = normalize_keymap(input.get("keymap")).bindings;
    config.keyword_highlight = serde_json::from_value(serde_json::to_value(
        normalize_keyword_highlight_config(&input["keyword_highlight"]),
    )?)?;
    Ok(())
}
