use std::fs;
use std::path::PathBuf;

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(target_os = "windows")]
fn resolve_config_dir() -> Result<PathBuf, String> {
    if let Some(appdata) = env_path("APPDATA") {
        return Ok(appdata.join("tterm"));
    }

    if let Some(userprofile) = env_path("USERPROFILE") {
        return Ok(userprofile.join("AppData").join("Roaming").join("tterm"));
    }

    if let (Some(home_drive), Some(home_path)) = (env_path("HOMEDRIVE"), env_path("HOMEPATH")) {
        let mut combined = PathBuf::new();
        combined.push(home_drive);
        combined.push(home_path);
        return Ok(combined.join("AppData").join("Roaming").join("tterm"));
    }

    if let Some(home) = env_path("HOME") {
        return Ok(home.join(".config").join("tterm"));
    }

    Err("Failed to resolve config directory: APPDATA/USERPROFILE/HOME are not set".to_string())
}

#[cfg(not(target_os = "windows"))]
fn resolve_config_dir() -> Result<PathBuf, String> {
    if let Some(xdg_config_home) = env_path("XDG_CONFIG_HOME") {
        return Ok(xdg_config_home.join("tterm"));
    }

    if let Some(home) = env_path("HOME") {
        return Ok(home.join(".config").join("tterm"));
    }

    Err("Failed to resolve config directory: HOME is not set".to_string())
}

pub fn get_config_path() -> Result<PathBuf, String> {
    resolve_config_dir()
}

pub fn ensure_config_dir() -> Result<PathBuf, String> {
    let config_dir = get_config_path()?;
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    Ok(config_dir)
}
