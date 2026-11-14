//! Settings read from the config file, brought back into the range the app
//! accepts.

use serde::{Deserialize, Deserializer};

use super::defaults::{
    default_predictive_echo, default_profile_sort, default_sftp_external_upload,
    default_tab_width_mode,
};

pub(super) fn deserialize_ui_scale_percent<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(((value / 10.0).round() * 10.0).clamp(80.0, 200.0) as u16)
}

pub(super) fn deserialize_window_blur_radius<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(value.round().clamp(1.0, 60.0) as u8)
}

pub(super) fn deserialize_window_opacity_percent<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(value.round().clamp(30.0, 95.0) as u8)
}

pub(super) fn deserialize_terminal_line_height<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(((value * 100.0).round() / 100.0).clamp(1.0, 2.0))
}

pub(super) fn deserialize_terminal_letter_spacing<'de, D>(deserializer: D) -> Result<i8, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(value.round().clamp(-5.0, 10.0) as i8)
}

pub(super) fn deserialize_notify_command_min_secs<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(value.round().clamp(1.0, 3600.0) as u16)
}

pub(super) fn deserialize_toast_max_visible<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(value.round().clamp(1.0, 5.0) as u8)
}

pub(super) fn deserialize_tab_width_mode<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let mode = String::deserialize(deserializer)?;
    Ok(if mode == "standard" {
        mode
    } else {
        default_tab_width_mode()
    })
}

pub(super) fn deserialize_profile_sort<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let sort = String::deserialize(deserializer)?;
    Ok(if sort == "recent" {
        sort
    } else {
        default_profile_sort()
    })
}

pub(super) fn deserialize_sftp_external_upload<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let mode = String::deserialize(deserializer)?;
    Ok(if mode == "auto" {
        mode
    } else {
        default_sftp_external_upload()
    })
}

pub(super) fn deserialize_tab_standard_width<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let width = f64::deserialize(deserializer)?;
    Ok(width.round().clamp(80.0, 300.0) as u16)
}

pub(super) fn deserialize_predictive_echo<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let mode = String::deserialize(deserializer)?;
    Ok(if mode == "always" || mode == "off" {
        mode
    } else {
        default_predictive_echo()
    })
}

pub(super) fn deserialize_predictive_echo_threshold_ms<'de, D>(
    deserializer: D,
) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(value.round().clamp(10.0, 2000.0) as u16)
}
