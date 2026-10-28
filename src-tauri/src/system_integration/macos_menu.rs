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

/// The workflow: one "Run Shell Script" action that takes the chosen folders
/// as its arguments and hands them to tTerm without waiting for it.
fn document_wflow(exe: &str) -> String {
    let script = xml_escape(&format!(
        "{} \"$@\" </dev/null >/dev/null 2>&1 &",
        sh_quote(exe)
    ));
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AMApplicationBuild</key>
	<string>523</string>
	<key>AMApplicationVersion</key>
	<string>2.10</string>
	<key>AMDocumentVersion</key>
	<string>2</string>
	<key>actions</key>
	<array>
		<dict>
			<key>action</key>
			<dict>
				<key>AMAccepts</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Optional</key>
					<true/>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>AMActionVersion</key>
				<string>2.0.3</string>
				<key>AMApplication</key>
				<array>
					<string>Automator</string>
				</array>
				<key>AMParameterProperties</key>
				<dict>
					<key>COMMAND_STRING</key>
					<dict/>
					<key>CheckedForUserDefaultShell</key>
					<dict/>
					<key>inputMethod</key>
					<dict/>
					<key>shell</key>
					<dict/>
					<key>source</key>
					<dict/>
				</dict>
				<key>AMProvides</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>ActionBundlePath</key>
				<string>/System/Library/Automator/Run Shell Script.action</string>
				<key>ActionName</key>
				<string>Run Shell Script</string>
				<key>ActionParameters</key>
				<dict>
					<key>COMMAND_STRING</key>
					<string>{script}</string>
					<key>CheckedForUserDefaultShell</key>
					<true/>
					<key>inputMethod</key>
					<integer>1</integer>
					<key>shell</key>
					<string>/bin/sh</string>
					<key>source</key>
					<string></string>
				</dict>
				<key>BundleIdentifier</key>
				<string>com.apple.RunShellScript</string>
				<key>CFBundleVersion</key>
				<string>2.0.3</string>
				<key>CanShowSelectedItemsWhenRun</key>
				<false/>
				<key>CanShowWhenRun</key>
				<true/>
				<key>Category</key>
				<array>
					<string>AMCategoryUtilities</string>
				</array>
				<key>Class Name</key>
				<string>RunShellScriptAction</string>
				<key>InputUUID</key>
				<string>5D0D3B2C-6E4B-4C51-9A8E-2B7A9E1C3F01</string>
				<key>Keywords</key>
				<array>
					<string>Shell</string>
					<string>Script</string>
					<string>Command</string>
					<string>Run</string>
					<string>Unix</string>
				</array>
				<key>OutputUUID</key>
				<string>8A6C2E4F-1B3D-4F7A-B9C1-6E2D8F0A4B02</string>
				<key>UUID</key>
				<string>C3E1F5A7-9B2D-4E6F-8A1C-3D5B7F9E1A03</string>
				<key>UnlocalizedApplications</key>
				<array>
					<string>Automator</string>
				</array>
				<key>isViewVisible</key>
				<integer>1</integer>
				<key>location</key>
				<string>309.000000:253.000000</string>
				<key>nibPath</key>
				<string>/System/Library/Automator/Run Shell Script.action/Contents/Resources/Base.lproj/main.nib</string>
			</dict>
			<key>isViewVisible</key>
			<integer>1</integer>
		</dict>
	</array>
	<key>connectors</key>
	<dict/>
	<key>workflowMetaData</key>
	<dict>
		<key>applicationBundleIDsByPath</key>
		<dict/>
		<key>applicationPaths</key>
		<array/>
		<key>inputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject.folder</string>
		<key>outputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>presentationMode</key>
		<integer>15</integer>
		<key>processesInput</key>
		<false/>
		<key>serviceApplicationBundleID</key>
		<string>com.apple.finder</string>
		<key>serviceApplicationPath</key>
		<string>/System/Library/CoreServices/Finder.app</string>
		<key>serviceInputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject.folder</string>
		<key>serviceOutputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>serviceProcessesInput</key>
		<false/>
		<key>systemImageName</key>
		<string>NSActionTemplate</string>
		<key>useAutomaticInputType</key>
		<false/>
		<key>workflowTypeIdentifier</key>
		<string>com.apple.Automator.servicesMenu</string>
	</dict>
</dict>
</plist>
"#
    )
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
