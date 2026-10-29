//! "Open in tTerm" in Finder's menu for a folder: a Quick Action, the
//! Automator workflow macOS lists under Quick Actions (or Services) for the
//! folders chosen. It runs tTerm's executable with their paths.

use std::path::{Path, PathBuf};
use std::process::Command;

use tauri::{AppHandle, Manager};

use super::launcher::{sh_quote, write_if_changed};
use super::FOLDER_MENU_LABELS;

/// The menu item: a workflow in the user's ~/Library/Services. It takes its
/// name from its file, so one in another language is removed.
pub struct FolderMenu {
    services: PathBuf,
}

impl FolderMenu {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let home = app
            .path()
            .home_dir()
            .map_err(|error| format!("Failed to find your home folder: {error}"))?;
        Ok(Self {
            services: home.join("Library").join("Services"),
        })
    }

    pub fn is_installed(&self) -> Result<bool, String> {
        Ok(FOLDER_MENU_LABELS
            .iter()
            .any(|(_, label)| workflow_path(&self.services, label).exists()))
    }

    /// Adds the menu item, named `label`, or removes it; returns whether it
    /// is there now.
    pub fn set_installed(&self, installed: bool, label: &str) -> Result<bool, String> {
        if installed {
            write(&self.services, label)?;
        } else {
            remove_except(&self.services, None)?;
            update_services();
        }
        Ok(installed)
    }

    /// Brings an installed menu item up to date: its name follows the app's
    /// language, and its script names tTerm's executable, which may move.
    pub fn refresh(&self, label: &str) -> Result<(), String> {
        if self.is_installed()? {
            write(&self.services, label)?;
        }
        Ok(())
    }
}

fn write(services: &Path, label: &str) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|error| format!("Failed to find tTerm's executable: {error}"))?;
    let contents = workflow_path(services, label).join("Contents");
    std::fs::create_dir_all(&contents)
        .map_err(|error| format!("Failed to create '{}': {error}", contents.display()))?;
    let renamed = remove_except(services, Some(label))?;
    let info = write_if_changed(&contents.join("Info.plist"), &info_plist(label))?;
    let document = write_if_changed(
        &contents.join("document.wflow"),
        &document_wflow(&exe.to_string_lossy()),
    )?;
    if renamed || info || document {
        update_services();
    }
    Ok(())
}

fn workflow_path(services: &Path, label: &str) -> PathBuf {
    services.join(format!("{label}.workflow"))
}

/// Removes tTerm's workflows other than the one named `keep`; returns
/// whether there was one.
fn remove_except(services: &Path, keep: Option<&str>) -> Result<bool, String> {
    let mut removed = false;
    let others = FOLDER_MENU_LABELS
        .iter()
        .map(|(_, label)| *label)
        .filter(|label| Some(*label) != keep);
    for label in others {
        let path = workflow_path(services, label);
        match std::fs::remove_dir_all(&path) {
            Ok(()) => removed = true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!("Failed to delete '{}': {error}", path.display()));
            }
        }
    }
    Ok(removed)
}

/// Has macOS read the Services folder again, so the change shows at once.
/// It runs on a thread of its own, since tTerm may be starting.
fn update_services() {
    std::thread::spawn(|| {
        if let Err(error) = Command::new("/System/Library/CoreServices/pbs")
            .arg("-update")
            .status()
        {
            eprintln!("Failed to refresh the Services menu: {error}");
        }
    });
}

fn info_plist(label: &str) -> String {
    let label = xml_escape(label);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>NSServices</key>
	<array>
		<dict>
			<key>NSMenuItem</key>
			<dict>
				<key>default</key>
				<string>{label}</string>
			</dict>
			<key>NSMessage</key>
			<string>runWorkflowAsService</string>
			<key>NSRequiredContext</key>
			<dict>
				<key>NSApplicationIdentifier</key>
				<string>com.apple.finder</string>
			</dict>
			<key>NSSendFileTypes</key>
			<array>
				<string>public.folder</string>
			</array>
		</dict>
	</array>
</dict>
</plist>
"#
    )
}

/// The workflow's plist, with `{script}` where the shell script goes.
const DOCUMENT_WFLOW: &str = include_str!("document.wflow");

/// The workflow: one "Run Shell Script" action that takes the chosen folders
/// as its arguments and hands them to tTerm without waiting for it.
fn document_wflow(exe: &str) -> String {
    let script = xml_escape(&format!(
        "{} \"$@\" </dev/null >/dev/null 2>&1 &",
        sh_quote(exe)
    ));
    DOCUMENT_WFLOW.replace("{script}", &script)
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_workflow_runs_tterm_with_the_chosen_folders() {
        let document = document_wflow("/Applications/tTerm.app/Contents/MacOS/tTerm");
        assert!(document.contains(
            "<string>&apos;/Applications/tTerm.app/Contents/MacOS/tTerm&apos; \
             &quot;$@&quot; &lt;/dev/null &gt;/dev/null 2&gt;&amp;1 &amp;</string>"
        ));
    }

    #[test]
    fn the_menu_item_is_named_in_the_language_given() {
        assert!(info_plist("在 tTerm 中打开").contains("<string>在 tTerm 中打开</string>"));
    }
}
