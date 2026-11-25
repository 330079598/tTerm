//! The debug-only SSH field entry that opens one pinned session without
//! starting the automation endpoint.
//!
//! The example reads one password from stdin and calls
//! [`super::run_ssh_tui_field`]. The password stays in memory, is injected
//! into the connection that the normal SSH tab opens, and is zeroed before the
//! TUI driver starts. The profile must already be seeded and its host key
//! confirmed in the numbered instance.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui_kit::{AnyWindowHandle, App, Entity};
use zeroize::Zeroizing;

use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::ssh::host_key_policy;

use super::command::CommandSpec;
use super::runtime;
use super::window::{self, Prototype};
use crate::check_instance;

static ARMED: AtomicBool = AtomicBool::new(false);
static PASSWORD: Mutex<Option<Zeroizing<String>>> = Mutex::new(None);
static OUTPUT: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Matches the schema written by `window::ssh_tui_field::report`.
const REPORT_SCHEMA: &str = "TTERM.gpui.ssh_tui_field.v1";

/// Whether this process is waiting to open the field SSH tab.
pub(super) fn is_armed() -> bool {
    ARMED.load(Ordering::SeqCst)
}

/// Adds the stdin password to the fixed profile's connection, and only that
/// profile.
///
/// The normal tab builder asks for these options more than once, so the value
/// is cloned as needed and cleared after the tab exists.
pub(super) fn inject_password(options: &mut PtyConnectionOptions) -> Result<(), String> {
    if !is_armed() {
        return Ok(());
    }
    if options.profile_id.as_deref() != Some(check_instance::PROFILE_ID) {
        return Err(
            "The SSH field entry may only open the fixed check profile in this instance."
                .to_string(),
        );
    }
    if options.password.is_some() {
        return Ok(());
    }
    let guard = PASSWORD
        .lock()
        .map_err(|_| "The SSH field password state is unavailable.".to_string())?;
    if let Some(password) = guard.as_ref() {
        options.password = Some(password.to_string());
    }
    Ok(())
}

/// The empty directory the parent prepared for field captures.
pub(super) fn output_dir() -> Option<PathBuf> {
    OUTPUT.lock().ok().and_then(|output| output.clone())
}

/// Hands the active SSH session to the TUI driver after the password is gone.
pub(super) fn after_window(handle: AnyWindowHandle, prototype: Entity<Prototype>, cx: &mut App) {
    if !ARMED.swap(false, Ordering::SeqCst) {
        return;
    }
    clear_password();
    super::window::ssh_tui_field::start(handle, prototype, cx);
}

/// Validates the numbered instance and fixed profile, then runs one window.
pub(super) fn run(password: Zeroizing<String>, output: Option<PathBuf>) -> Result<(), String> {
    let identifier = check_instance::require_numbered_instance()?;
    isolate_home(&identifier)?;
    let output = prepare_output(output)?;

    let runtime = runtime::start()?;
    let profile = check_instance::field_profile()?;
    check_instance::require_pinned_host(&profile)?;
    host_key_policy::require_pinned()?;

    let report_directory = output.clone();
    set_password(password)?;
    set_output(output)?;
    ARMED.store(true, Ordering::SeqCst);

    let command = CommandSpec::SshProfile {
        selector: profile.id,
    };
    window::run_window(command, runtime.services, runtime.events);
    ARMED.store(false, Ordering::SeqCst);
    clear_password();
    verify_report(report_directory.as_deref())?;
    drop(runtime._runtime);
    Ok(())
}

/// Fails the run unless this invocation left a complete passing report.
fn verify_report(directory: Option<&Path>) -> Result<(), String> {
    let Some(directory) = directory else {
        return Ok(());
    };
    let path = directory.join("report.json");
    let text = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "The field run did not leave a readable report at '{}': {error}",
            path.display()
        )
    })?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
        format!(
            "The field report '{}' is not valid JSON: {error}",
            path.display()
        )
    })?;
    let report = value.as_object().ok_or_else(|| {
        format!(
            "The field report '{}' is not a JSON object.",
            path.display()
        )
    })?;
    let schema = report.get("schema").and_then(|value| value.as_str());
    if schema != Some(REPORT_SCHEMA) {
        return Err(format!(
            "The field report '{}' has schema {schema:?}, expected '{REPORT_SCHEMA}'.",
            path.display()
        ));
    }
    let outcome = report.get("outcome").and_then(|value| value.as_str());
    if outcome != Some("pass") {
        let failure = report
            .get("failure")
            .and_then(|value| value.as_str())
            .unwrap_or("no failure detail");
        return Err(format!(
            "The field report '{}' outcome is {outcome:?}: {failure}",
            path.display()
        ));
    }
    if !report
        .get("failure")
        .is_some_and(serde_json::Value::is_null)
    {
        return Err(format!(
            "The field report '{}' says pass but carries a failure.",
            path.display()
        ));
    }
    let stages = report
        .get("stages")
        .and_then(|value| value.as_array())
        .ok_or_else(|| format!("The field report '{}' has no stages array.", path.display()))?;
    if stages.is_empty() {
        return Err(format!(
            "The field report '{}' has no stages.",
            path.display()
        ));
    }
    for stage in stages {
        let stage = stage.as_object().ok_or_else(|| {
            format!(
                "The field report '{}' has a stage that is not an object.",
                path.display()
            )
        })?;
        let name = stage
            .get("name")
            .and_then(|value| value.as_str())
            .ok_or_else(|| {
                format!(
                    "The field report '{}' has a stage without a name.",
                    path.display()
                )
            })?;
        if stage.get("passed").and_then(|value| value.as_bool()) != Some(true) {
            return Err(format!(
                "The field report '{}' stage '{name}' did not pass.",
                path.display()
            ));
        }
        let screenshots_are_strings = stage
            .get("screenshots")
            .and_then(|value| value.as_array())
            .is_some_and(|screenshots| screenshots.iter().all(serde_json::Value::is_string));
        if stage
            .get("detail")
            .and_then(|value| value.as_str())
            .is_none()
            || stage
                .get("observed")
                .and_then(|value| value.as_str())
                .is_none()
            || !screenshots_are_strings
        {
            return Err(format!(
                "The field report '{}' stage '{name}' is malformed.",
                path.display()
            ));
        }
    }
    Ok(())
}

/// Points `HOME` at a private directory inside the numbered instance so the
/// OpenSSH compatibility layer cannot read the operator's `~/.ssh` files.
fn isolate_home(identifier: &str) -> Result<(), String> {
    let root = check_instance::instance_root_in(&std::env::temp_dir(), identifier);
    if !root.is_dir() {
        return Err(format!(
            "The numbered instance '{}' does not exist; seed it in a new instance first.",
            root.display()
        ));
    }
    let home = root.join("home");
    std::fs::create_dir_all(&home).map_err(|error| {
        format!(
            "Failed to create the field home '{}': {error}",
            home.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).map_err(
            |error| {
                format!(
                    "Failed to make the field home private '{}': {error}",
                    home.display()
                )
            },
        )?;
    }
    // SAFETY: the caller runs this from `main` before the runtime starts any
    // thread, so no other thread can read the environment while it changes.
    unsafe {
        std::env::set_var("HOME", &home);
    }
    Ok(())
}

/// The parent creates the capture directory and leaves it empty.
fn prepare_output(output: Option<PathBuf>) -> Result<Option<PathBuf>, String> {
    let Some(directory) = output else {
        return Ok(None);
    };
    let mut entries = std::fs::read_dir(&directory).map_err(|error| {
        format!(
            "The SSH field output directory '{}' cannot be read: {error}",
            directory.display()
        )
    })?;
    if entries.next().is_some() {
        return Err(format!(
            "The SSH field output directory '{}' must be empty.",
            directory.display()
        ));
    }
    Ok(Some(directory))
}

fn set_password(password: Zeroizing<String>) -> Result<(), String> {
    let mut guard = PASSWORD
        .lock()
        .map_err(|_| "The SSH field password state is unavailable.".to_string())?;
    *guard = Some(password);
    Ok(())
}

fn set_output(output: Option<PathBuf>) -> Result<(), String> {
    let mut guard = OUTPUT
        .lock()
        .map_err(|_| "The SSH field output state is unavailable.".to_string())?;
    *guard = output;
    Ok(())
}

fn clear_password() {
    if let Ok(mut guard) = PASSWORD.lock() {
        drop(guard.take());
    }
}

#[cfg(test)]
mod tests {
    use super::{REPORT_SCHEMA, prepare_output, verify_report};

    #[test]
    fn an_absent_output_directory_is_allowed() {
        assert_eq!(prepare_output(None).expect("no output"), None);
    }

    #[test]
    fn an_existing_empty_output_directory_is_accepted() {
        let root =
            std::env::temp_dir().join(format!("tterm-gpui-field-output-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create the output directory");

        let prepared = prepare_output(Some(root.clone())).expect("empty output directory");
        assert_eq!(prepared.as_deref(), Some(root.as_path()));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_non_empty_output_directory_is_rejected() {
        let root = std::env::temp_dir().join(format!(
            "tterm-gpui-field-output-{}-full",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create the output directory");
        std::fs::write(root.join("capture.png"), b"x").expect("write a marker");

        let error = prepare_output(Some(root.clone())).expect_err("non-empty output");
        assert!(error.contains("must be empty"), "{error}");

        let _ = std::fs::remove_dir_all(&root);
    }

    fn report_directory(suffix: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tterm-gpui-field-report-{}-{suffix}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create the report directory");
        root
    }

    fn passing_report() -> serde_json::Value {
        serde_json::json!({
            "schema": REPORT_SCHEMA,
            "outcome": "pass",
            "failure": null,
            "output_dir": "/tmp/output",
            "stages": [{
                "name": "connection",
                "passed": true,
                "detail": "connected",
                "observed": "shell",
                "screenshots": ["connection.png"],
            }],
        })
    }

    #[test]
    fn a_complete_passing_report_is_accepted() {
        let root = report_directory("pass");
        std::fs::write(
            root.join("report.json"),
            serde_json::to_string(&passing_report()).unwrap(),
        )
        .expect("write the report");

        verify_report(Some(&root)).expect("the report passes");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_or_failing_report_is_rejected() {
        let missing = report_directory("missing");
        assert!(verify_report(Some(&missing)).is_err());
        let _ = std::fs::remove_dir_all(&missing);

        let failed = report_directory("fail");
        let mut report = passing_report();
        report["outcome"] = serde_json::json!("fail");
        report["failure"] = serde_json::json!("stage 'nvim' failed");
        std::fs::write(
            failed.join("report.json"),
            serde_json::to_string(&report).unwrap(),
        )
        .expect("write the report");
        assert!(verify_report(Some(&failed)).is_err());
        let _ = std::fs::remove_dir_all(&failed);
    }

    #[test]
    fn a_wrong_schema_or_failed_stage_is_rejected() {
        let wrong_schema = report_directory("schema");
        let mut report = passing_report();
        report["schema"] = serde_json::json!("TTERM.gpui.ssh_tui_field.v0");
        std::fs::write(
            wrong_schema.join("report.json"),
            serde_json::to_string(&report).unwrap(),
        )
        .expect("write the report");
        assert!(verify_report(Some(&wrong_schema)).is_err());
        let _ = std::fs::remove_dir_all(&wrong_schema);

        let failed_stage = report_directory("stage");
        let mut report = passing_report();
        report["stages"][0]["passed"] = serde_json::json!(false);
        std::fs::write(
            failed_stage.join("report.json"),
            serde_json::to_string(&report).unwrap(),
        )
        .expect("write the report");
        assert!(verify_report(Some(&failed_stage)).is_err());
        let _ = std::fs::remove_dir_all(&failed_stage);
    }
}
