use super::types::TunnelRule;
use crate::config::{atomic_write, ensure_config_dir, get_config_path};
use std::fs;

const TUNNELS_FILE: &str = "tunnels.json";

pub(crate) fn load_tunnels_from_disk() -> Result<Vec<TunnelRule>, String> {
    let path = get_config_path()?.join(TUNNELS_FILE);
    if !path.exists() {
        return Ok(vec![]);
    }
    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read tunnels file: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse tunnels: {e}"))
}

pub(crate) fn write_tunnels_to_disk(tunnels: &[TunnelRule]) -> Result<(), String> {
    let path = ensure_config_dir()?.join(TUNNELS_FILE);
    let content = serde_json::to_string_pretty(tunnels)
        .map_err(|e| format!("Failed to serialize tunnels: {e}"))?;
    atomic_write(&path, content)
}
