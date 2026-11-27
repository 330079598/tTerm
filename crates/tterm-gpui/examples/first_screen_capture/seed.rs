//! Builds one disposable numbered instance for the first-screen capture.
//!
//! The instance lives under the capture's own base directory. It holds a
//! synthetic configuration, four synthetic saved SSH profiles and their
//! groups, a fixed window size and a shell script that prints fixed bytes.
//! Nothing here reads a user's configuration, database, keychain or network.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;
use tterm_core::config::AppConfig;
use tterm_core::db::Database;
use tterm_core::db::meta;
use tterm_core::profiles::groups::replace_groups;
use tterm_core::profiles::storage::replace_profiles;
use tterm_core::profiles::types::{ProfileGroup, SavedProfile};

const IDENTIFIER: &str = "com.stone.tTerm.gpui-dev";

pub struct Instance {
    pub number: u32,
    pub root: PathBuf,
    pub script: PathBuf,
}

pub fn create(
    base: &Path,
    number: u32,
    theme: &str,
    width: u32,
    height: u32,
    fixture_root: &Path,
) -> Result<Instance, String> {
    let root = base.join(format!("{IDENTIFIER}.check-{number}"));
    create_private_dir(&root)?;
    let config_dir = root.join("config");
    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("could not create {}: {error}", config_dir.display()))?;

    let database = Database::open_in_config_dir(&config_dir)?;
    let profiles: Vec<SavedProfile> =
        fixture_command(fixture_root, "first-screen.profiles.json", "list_profiles")?;
    let groups: Vec<ProfileGroup> = fixture_command(
        fixture_root,
        "first-screen.profiles.json",
        "list_profile_groups",
    )?;
    let terminal_fixture: serde_json::Value =
        fixture_json(fixture_root, "first-screen.terminal.json")?;
    let lines: Vec<String> = serde_json::from_value(terminal_fixture["lines"].clone())
        .map_err(|error| format!("first-screen terminal lines are invalid: {error}"))?;
    let background = if theme == "light" {
        "#ffffff"
    } else {
        "#1b1d23"
    };
    database.write(|transaction| {
        replace_groups(transaction, &groups)?;
        replace_profiles(transaction, &profiles)?;
        meta::set(transaction, "window_background", background)?;
        meta::set(
            transaction,
            "gpui_window_state",
            &json!({
                "bounds": { "x": 120.0, "y": 80.0, "width": width, "height": height },
                "maximized": false
            }),
        )?;
        Ok(())
    })?;

    write_config(&config_dir, theme)?;
    let script = write_terminal_fixture(&root, &lines)?;
    Ok(Instance {
        number,
        root,
        script,
    })
}

fn fixture_json<T: serde::de::DeserializeOwned>(root: &Path, name: &str) -> Result<T, String> {
    let path = root.join("parity").join("fixtures").join(name);
    let bytes =
        fs::read(&path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("could not parse {}: {error}", path.display()))
}

fn fixture_command<T: serde::de::DeserializeOwned>(
    root: &Path,
    name: &str,
    command: &str,
) -> Result<T, String> {
    let value: serde_json::Value = fixture_json(root, name)?;
    let answer = value["commands"][command].clone();
    if answer.is_null() {
        return Err(format!("{name} has no {command} fixture"));
    }
    serde_json::from_value(answer)
        .map_err(|error| format!("{name} {command} does not match its type: {error}"))
}

fn write_config(config_dir: &Path, theme: &str) -> Result<(), String> {
    let config = AppConfig {
        theme: if theme == "light" { "light" } else { "default" }.to_string(),
        theme_follow_system: false,
        theme_light: "light".to_string(),
        theme_dark: "default".to_string(),
        language: "en".to_string(),
        ui_scale_percent: 100,
        ..AppConfig::default()
    };
    let path = config_dir.join("config.json");
    let bytes = serde_json::to_vec_pretty(&config)
        .map_err(|error| format!("could not encode {}: {error}", path.display()))?;
    fs::write(&path, bytes).map_err(|error| format!("could not write {}: {error}", path.display()))
}

fn write_terminal_fixture(root: &Path, lines: &[String]) -> Result<PathBuf, String> {
    let output = root.join("terminal-output.bin");
    // The last line is the waiting prompt, so it gets no final newline: the
    // same bytes the reference writes keep the cursor on that line.
    let bytes = lines.join("\r\n").into_bytes();
    fs::write(&output, bytes)
        .map_err(|error| format!("could not write {}: {error}", output.display()))?;
    let script = root.join("terminal-fixture.sh");
    let body =
        "#!/bin/sh\ncat \"$(dirname \"$0\")/terminal-output.bin\"\nwhile :; do sleep 60; done\n";
    fs::write(&script, body)
        .map_err(|error| format!("could not write {}: {error}", script.display()))?;
    make_executable(&script)?;
    Ok(script)
}

fn create_private_dir(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder
            .create(path)
            .map_err(|error| format!("could not create {}: {error}", path.display()))
    }
    #[cfg(not(unix))]
    {
        fs::create_dir(path)
            .map_err(|error| format!("could not create {}: {error}", path.display()))
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("could not make {} executable: {error}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Err("the first-screen capture harness currently runs on unix only".to_string())
}
