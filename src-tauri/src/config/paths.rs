use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

static CONFIG_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn init_config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to resolve app config directory: {}", e))?;

    if let Some(existing) = CONFIG_DIR.get() {
        if existing != &config_dir {
            return Err(format!(
                "Config directory already initialized to '{}' but attempted to reinitialize to '{}'",
                existing.display(),
                config_dir.display()
            ));
        }
        return Ok(existing.clone());
    }

    CONFIG_DIR
        .set(config_dir.clone())
        .map_err(|_| "Failed to initialize config directory".to_string())?;

    Ok(config_dir)
}

pub fn get_config_path() -> Result<PathBuf, String> {
    CONFIG_DIR.get().cloned().ok_or_else(|| {
        "Config directory is not initialized. Call init_config_dir during setup before accessing persisted state."
            .to_string()
    })
}

pub fn ensure_config_dir() -> Result<PathBuf, String> {
    let config_dir = get_config_path()?;
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    Ok(config_dir)
}

pub fn legacy_config_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let appdata = appdata.trim();
            if !appdata.is_empty() {
                return Ok(PathBuf::from(appdata).join("tterm"));
            }
        }

        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let userprofile = userprofile.trim();
            if !userprofile.is_empty() {
                return Ok(PathBuf::from(userprofile)
                    .join("AppData")
                    .join("Roaming")
                    .join("tterm"));
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg_config_home) = std::env::var("XDG_CONFIG_HOME") {
            let xdg_config_home = xdg_config_home.trim();
            if !xdg_config_home.is_empty() {
                return Ok(PathBuf::from(xdg_config_home).join("tterm"));
            }
        }

        if let Ok(home) = std::env::var("HOME") {
            let home = home.trim();
            if !home.is_empty() {
                return Ok(PathBuf::from(home).join(".config").join("tterm"));
            }
        }
    }

    Err("Failed to resolve legacy config directory from environment".to_string())
}
