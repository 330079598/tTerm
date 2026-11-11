//! What the user last chose in the settings for each of tTerm's additions to
//! Windows. Uninstalling takes them out, and so does the uninstall Tauri's
//! installer runs before it installs a new version over an old one, so the
//! installer puts back what the user had chosen. The choices live in tTerm's
//! local data directory, which the uninstaller deletes only when the user asks
//! it to delete the app data as well.

use std::path::Path;

/// One of tTerm's additions to Windows.
#[derive(Clone, Copy, Debug)]
pub enum Addition {
    ShellCommand,
    FolderMenu,
}

impl Addition {
    fn file_name(self) -> &'static str {
        match self {
            Self::ShellCommand => "tterm-command.choice",
            Self::FolderMenu => "folder-menu.choice",
        }
    }

    /// Whether the installer adds it for a user who has never chosen.
    fn wanted_by_default(self) -> bool {
        matches!(self, Self::ShellCommand)
    }
}

/// Whether the user wants `addition`: as they last chose, or by default.
pub fn wanted(dir: &Path, addition: Addition) -> bool {
    std::fs::read_to_string(dir.join(addition.file_name()))
        .ok()
        .and_then(|choice| match choice.trim() {
            "on" => Some(true),
            "off" => Some(false),
            _ => None,
        })
        .unwrap_or_else(|| addition.wanted_by_default())
}

/// Whether the user has chosen for `addition` yet.
pub fn is_chosen(dir: &Path, addition: Addition) -> bool {
    dir.join(addition.file_name()).exists()
}

/// Remembers that the user turned `addition` on or off.
pub fn remember(dir: &Path, addition: Addition, on: bool) -> Result<(), String> {
    let path = dir.join(addition.file_name());
    std::fs::create_dir_all(dir)
        .and_then(|()| std::fs::write(&path, if on { "on" } else { "off" }))
        .map_err(|error| format!("Failed to write '{}': {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_choice_wins_over_the_default() {
        let dir = std::env::temp_dir().join(format!("tterm-choices-{}", uuid::Uuid::new_v4()));
        assert!(wanted(&dir, Addition::ShellCommand));
        assert!(!wanted(&dir, Addition::FolderMenu));
        assert!(!is_chosen(&dir, Addition::FolderMenu));

        remember(&dir, Addition::ShellCommand, false).unwrap();
        remember(&dir, Addition::FolderMenu, true).unwrap();
        assert!(!wanted(&dir, Addition::ShellCommand));
        assert!(wanted(&dir, Addition::FolderMenu));
        assert!(is_chosen(&dir, Addition::FolderMenu));

        remember(&dir, Addition::ShellCommand, true).unwrap();
        assert!(wanted(&dir, Addition::ShellCommand));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
