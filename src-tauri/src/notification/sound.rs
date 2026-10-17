//! The audio file picked as the notification sound. tTerm plays its own copy
//! in the config folder, so the sound outlives the original file being moved
//! and backups and sync can take it to other devices.

use std::fs;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use crate::config::atomic::atomic_write;
use crate::config::paths::{ensure_config_dir, get_config_path};

/// The copy's name in the config folder.
pub(crate) const SOUND_FILE: &str = "notification-sound";
/// Largest sound accepted: a notification sound is a short clip, and every
/// sync uploads it.
const MAX_SOUND_FILE_BYTES: usize = 1024 * 1024;

fn sound_path() -> Result<PathBuf, String> {
    Ok(get_config_path()?.join(SOUND_FILE))
}

fn save(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_SOUND_FILE_BYTES {
        return Err("The sound file is over 1 MB; pick a short sound.".to_string());
    }
    atomic_write(&ensure_config_dir()?.join(SOUND_FILE), bytes)
}

/// Copies the file at `path` in as the notification sound and returns its
/// name, which the settings show.
pub(super) fn import(path: &Path) -> Result<String, String> {
    let size = fs::metadata(path)
        .map_err(|error| format!("Couldn't open the sound file {}: {error}", path.display()))?
        .len();
    if size > MAX_SOUND_FILE_BYTES as u64 {
        return Err(format!(
            "The sound file {} is over 1 MB; pick a short sound.",
            path.display()
        ));
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("Couldn't read the sound file {}: {error}", path.display()))?;
    save(&bytes)?;
    Ok(path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default())
}

pub(super) fn read() -> Result<Vec<u8>, String> {
    fs::read(sound_path()?).map_err(|error| {
        format!("Couldn't read the notification sound; pick the sound file again: {error}")
    })
}

/// The sound as base64 for a backup or the sync document, `None` when no
/// file was ever picked.
pub(crate) fn export() -> Result<Option<String>, String> {
    let path = sound_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path)
        .map_err(|error| format!("Couldn't read the notification sound: {error}"))?;
    Ok(Some(BASE64.encode(bytes)))
}

/// Writes a sound that came from a backup or another device.
pub(crate) fn restore(encoded: &str) -> Result<(), String> {
    let bytes = BASE64
        .decode(encoded)
        .map_err(|error| format!("The notification sound is invalid: {error}"))?;
    save(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_restored_sound_over_the_limit_is_refused() {
        let encoded = BASE64.encode(vec![0u8; MAX_SOUND_FILE_BYTES + 1]);
        assert!(restore(&encoded).unwrap_err().contains("over 1 MB"));
    }

    #[test]
    fn a_restored_sound_that_is_not_base64_is_refused() {
        assert!(restore("not base64!").unwrap_err().contains("invalid"));
    }
}
