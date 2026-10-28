//! "Open in tTerm" in Explorer's menu for a folder, for the empty space inside
//! one and for a drive. Windows 11 lists it under "Show more options", where
//! programs that do not ship a shell extension appear.

use tauri::AppHandle;
use windows::Win32::System::Registry::REG_SZ;

use super::registry::{self, Key};

/// The verbs, under the current user's classes, so no administrator is needed.
const VERB_KEYS: [&str; 3] = [
    r"Software\Classes\Directory\shell\tTerm",
    r"Software\Classes\Directory\Background\shell\tTerm",
    r"Software\Classes\Drive\shell\tTerm",
];

/// The menu item in the current user's registry.
pub struct FolderMenu;

impl FolderMenu {
    pub fn new(_app: &AppHandle) -> Result<Self, String> {
        Ok(Self)
    }

    pub fn is_installed(&self) -> Result<bool, String> {
        Ok(Key::open(&format!(r"{}\command", VERB_KEYS[0]))?.is_some())
    }

    /// Adds the menu item, named `label`, or removes it; returns whether it
    /// is there now.
    pub fn set_installed(&self, installed: bool, label: &str) -> Result<bool, String> {
        if installed {
            write(label)?;
        } else {
            for path in VERB_KEYS {
                registry::delete_tree(path)?;
            }
        }
        Ok(installed)
    }

    /// Brings an installed menu item up to date: its name follows the app's
    /// language, and its command names tTerm's executable, which may move.
    pub fn refresh(&self, label: &str) -> Result<(), String> {
        if self.is_installed()? {
            write(label)?;
        }
        Ok(())
    }
}

fn write(label: &str) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|error| format!("Failed to find tTerm's executable: {error}"))?;
    let exe = exe.to_string_lossy();
    // Explorer replaces %V with the folder, or the drive's root.
    let command = format!("\"{exe}\" \"%V\"");
    for path in VERB_KEYS {
        let verb = Key::create(path)?;
        write_if_changed(&verb, None, label)?;
        write_if_changed(&verb, Some("Icon"), &exe)?;
        write_if_changed(&Key::create(&format!(r"{path}\command"))?, None, &command)?;
    }
    Ok(())
}

// Every settings change refreshes the menu, which rarely needs a write.
fn write_if_changed(key: &Key, name: Option<&str>, value: &str) -> Result<(), String> {
    if key
        .read_string(name)?
        .is_some_and(|(current, kind)| current == value && kind == REG_SZ)
    {
        return Ok(());
    }
    key.write_string(name, value, REG_SZ)
}
