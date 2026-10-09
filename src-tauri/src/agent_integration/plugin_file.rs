//! A file of tTerm's own that an agent loads from its config directory: an
//! OpenCode plugin, a pi extension. Installing writes it whole; removing
//! deletes it. A file at that path that does not report to tTerm belongs to
//! someone else and is left alone.

use std::fs;
use std::path::Path;

use super::{IntegrationState, REPORT_MARKER};

pub(super) struct PluginFile {
    /// Where the agent looks for it, under its config directory.
    pub path: &'static [&'static str],
    pub content: &'static str,
}

impl PluginFile {
    pub(super) fn state(&self, path: &Path) -> Result<IntegrationState, String> {
        match fs::read_to_string(path) {
            Ok(text) if text == self.content => Ok(IntegrationState::Installed),
            Ok(text) if text.contains(REPORT_MARKER) => Ok(IntegrationState::Outdated),
            Ok(_) => Err(format!("{} is not tTerm's", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(IntegrationState::NotInstalled)
            }
            Err(error) => Err(format!("Failed to read {}: {error}", path.display())),
        }
    }

    pub(super) fn install(&self, path: &Path) -> Result<(), String> {
        self.state(path)?;
        crate::config::atomic_write(path, self.content)
    }

    /// Also removes the folder it had to itself, when that is left empty.
    pub(super) fn uninstall(&self, path: &Path) -> Result<(), String> {
        if self.state(path)? == IntegrationState::NotInstalled {
            return Ok(());
        }
        fs::remove_file(path)
            .map_err(|error| format!("Failed to remove {}: {error}", path.display()))?;
        if self.path.len() > 2 {
            if let Some(folder) = path.parent() {
                // Fails, as it should, when something else is in it.
                let _ = fs::remove_dir(folder);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: PluginFile = PluginFile {
        path: &["plugins", "tterm-agent", "tui.js"],
        content: "write(\"\\x1b]777;tterm-agent;test;done\\x07\")\n",
    };

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tterm-agent-plugin-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn installs_updates_and_removes_its_own_file() {
        let dir = temp_dir("own");
        let path = FILE
            .path
            .iter()
            .fold(dir.clone(), |path, part| path.join(part));
        assert_eq!(FILE.state(&path), Ok(IntegrationState::NotInstalled));
        FILE.install(&path).unwrap();
        assert_eq!(FILE.state(&path), Ok(IntegrationState::Installed));

        fs::write(&path, "old \u{1b}]777;tterm-agent;test;done").unwrap();
        assert_eq!(FILE.state(&path), Ok(IntegrationState::Outdated));
        FILE.install(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), FILE.content);

        FILE.uninstall(&path).unwrap();
        assert!(!path.exists());
        assert!(!dir.join("plugins").join("tterm-agent").exists());
        assert!(dir.join("plugins").exists());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn leaves_someone_elses_file_alone() {
        let dir = temp_dir("foreign");
        let path = FILE
            .path
            .iter()
            .fold(dir.clone(), |path, part| path.join(part));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "export default {}").unwrap();
        assert!(FILE.install(&path).is_err());
        assert!(FILE.uninstall(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "export default {}");
        fs::remove_dir_all(dir).ok();
    }
}
