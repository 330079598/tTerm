//! Where the SSH config file is and how its `~` paths expand.

use std::env;
use std::path::PathBuf;

pub(crate) fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
        .or_else(|| {
            let drive = env::var_os("HOMEDRIVE")?;
            let path = env::var_os("HOMEPATH")?;
            Some(PathBuf::from(format!(
                "{}{}",
                drive.to_string_lossy(),
                path.to_string_lossy()
            )))
        })
}

pub(crate) fn expand_home_path(path: &str) -> PathBuf {
    let trimmed = path.trim().trim_matches('"');
    if trimmed == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(trimmed));
    }

    if let Some(rest) = trimmed
        .strip_prefix("~/")
        .or_else(|| trimmed.strip_prefix("~\\"))
    {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }

    PathBuf::from(trimmed)
}

pub(crate) fn normalize_import_source(source_path: Option<String>) -> Result<PathBuf, String> {
    let source = source_path
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(expand_home_path)
        .or_else(|| home_dir().map(|home| home.join(".ssh").join("config")))
        .ok_or_else(|| "Unable to locate the user home directory".to_string())?;

    let source = if source.is_dir() {
        source.join("config")
    } else {
        source
    };

    if !source.exists() {
        return Err(format!("SSH config file not found: {}", source.display()));
    }

    if !source.is_file() {
        return Err(format!(
            "SSH config path is not a file: {}",
            source.display()
        ));
    }

    Ok(source)
}
