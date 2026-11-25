#![cfg(all(debug_assertions, feature = "automation"))]

//! Field acceptance driver for one already-authenticated SSH TUI session.
//!
//! [`start`] is called by `field::after_window` after the password slot has
//! been cleared. It reads the active tab's existing binding, never opens
//! another tab and never dials again. The driver runs a fixed tmux and nvim
//! script inside one private temporary `HOME`, writes staged screenshots and
//! `report.json` into the directory the parent prepared, and then closes its
//! own window through the same close gate the automation endpoint uses.

mod observe;
mod report;
mod script;

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::{AnyWindowHandle, App, AsyncApp, Entity};

use super::Prototype;
use crate::terminal_demo::binding::TabBinding;
use crate::terminal_demo::binding::scoped::ConnectionState;
use crate::terminal_demo::field;
use crate::terminal_demo::session::Session;
use tterm_terminal::mode::Modes;
use tterm_terminal::terminal::Terminal;

const OUTPUT_ENV: &str = "TTERM_GPUI_FIELD_OUTPUT";
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(120);
const STAGE_TIMEOUT: Duration = Duration::from_secs(60);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const MODE_SETTLE: Duration = Duration::from_millis(500);

type InputSender = Box<dyn Fn(&[u8])>;

/// Which program currently owns the terminal's input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiMode {
    Shell,
    Tmux,
    Nvim,
}

struct Driver {
    handle: AnyWindowHandle,
    binding: Rc<TabBinding>,
    session: Arc<Session>,
    model: Arc<Terminal>,
    input: InputSender,
    output: PathBuf,
    mode: Cell<UiMode>,
}

impl Driver {
    /// Sends bytes only while the session is still connected and no host-key
    /// question is waiting. This is the same gate the field notes require
    /// before any keystroke is typed into the remote shell.
    fn send(&self, bytes: &[u8]) -> Result<(), String> {
        let state = self.binding.scoped_state();
        if state.host_key_prompt().is_some() {
            return Err(
                "A host-key question is waiting; the field driver never answers it.".to_string(),
            );
        }
        if state.connection_state() != ConnectionState::Connected {
            return Err(format!(
                "The SSH session is not connected (state {:?}).",
                state.connection_state()
            ));
        }
        if !self.session.is_live() {
            return Err("The SSH session input path is closed.".to_string());
        }
        (self.input)(bytes);
        Ok(())
    }

    fn set_mode(&self, mode: UiMode) {
        self.mode.set(mode);
    }
}

/// Runs the field capture on the active SSH tab.
///
/// The caller must have opened the normal SSH profile and prepared an empty
/// output directory; both are arranged by `field::run`.
pub(in crate::terminal_demo) fn start(
    handle: AnyWindowHandle,
    prototype: Entity<Prototype>,
    cx: &mut App,
) {
    let output = match field_output() {
        Ok(output) => output,
        Err(error) => {
            fail_start(handle, &prototype, cx, &error);
            return;
        }
    };
    let binding = match active_tab(&prototype, cx) {
        Ok(active) => active,
        Err(error) => {
            fail_start(handle, &prototype, cx, &error);
            return;
        }
    };
    let Some(session) = binding.session() else {
        fail_start(
            handle,
            &prototype,
            cx,
            "The active SSH tab has no session to drive.",
        );
        return;
    };
    let model = session.model();
    let input = Box::new(session.input());
    let driver = Driver {
        handle,
        binding,
        session,
        model,
        input,
        output: output.clone(),
        mode: Cell::new(UiMode::Shell),
    };
    let report = report::Report::new(output);
    cx.spawn(async move |cx| {
        drive(driver, prototype, report, cx).await;
    })
    .detach();
}

async fn drive(
    driver: Driver,
    prototype: Entity<Prototype>,
    mut report: report::Report,
    cx: &mut AsyncApp,
) {
    let script = run_script(&driver, &mut report, cx).await;
    let cleanup = cleanup(&driver, &mut report, cx).await;
    match (&script, &cleanup) {
        (Ok(()), Ok(())) => report.finish_success(),
        (Err(error), _) => report.finish_failure(error),
        (Ok(()), Err(error)) => report.finish_failure(error),
    }
    if let Err(error) = report.write() {
        eprintln!("tterm-gpui field: {error}");
    }
    let _ = driver.handle.update(cx, |_, window, app| {
        prototype.update(app, |prototype, cx| {
            prototype.request_close(window, cx);
        });
    });
}

async fn run_script(
    driver: &Driver,
    report: &mut report::Report,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    if let Err(error) = wait_for_connection(&driver.binding, cx).await {
        report.stage("connection", false, &error, "", Vec::new());
        return Err(format!("stage 'connection' failed: {error}"));
    }
    let connected_text = observe::visible_text(&driver.model);
    let connected_shot =
        match observe::capture(driver.handle, &driver.output, "01-connected.png", cx) {
            Ok(name) => name,
            Err(error) => {
                report.stage("connection", false, &error, &connected_text, Vec::new());
                return Err(format!("stage 'connection' failed: {error}"));
            }
        };
    report.stage(
        "connection",
        true,
        "The SSH profile is connected and no prompt or host-key question is waiting.",
        &connected_text,
        vec![connected_shot],
    );

    let mut enter = String::from(script::ENTER_POSIX_SHELL);
    enter.push('\r');
    if let Err(error) = driver.send(enter.as_bytes()) {
        return fail_stage(report, driver, "posix-shell", error, Vec::new());
    }
    cx.background_executor()
        .timer(Duration::from_millis(600))
        .await;
    run_shell_stage(driver, report, &script::shell_ready(), cx).await?;
    run_shell_stage(driver, report, &script::tmux_split(), cx).await?;
    run_tmux_attach(driver, report, cx).await?;
    run_shell_stage(driver, report, &script::tmux_verify_close(), cx).await?;
    run_nvim(driver, report, cx).await?;
    run_shell_stage(driver, report, &script::nvim_verify(), cx).await?;
    run_shell_stage(driver, report, &script::final_shell(), cx).await?;

    if driver.model.modes().contains(Modes::ALT_SCREEN) {
        let error =
            "The final shell marker arrived while the alternate screen was active.".to_string();
        let observed = observe::visible_text(&driver.model);
        report.stage("primary-screen", false, &error, &observed, Vec::new());
        return Err(format!("stage 'primary-screen' failed: {error}"));
    }
    Ok(())
}

async fn run_shell_stage(
    driver: &Driver,
    report: &mut report::Report,
    stage: &script::ShellStage,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let mut command = String::from(stage.command);
    command.push('\r');
    if let Err(error) = driver.send(command.as_bytes()) {
        return fail_stage(report, driver, stage.name, error, Vec::new());
    }
    let waited = observe::wait_for_marker(
        &driver.model,
        stage.sentinel,
        stage.ok,
        stage.fail,
        STAGE_TIMEOUT,
        cx,
    )
    .await;
    let observed = observe::visible_text(&driver.model);
    let screenshot = match observe::capture(driver.handle, &driver.output, stage.screenshot, cx) {
        Ok(name) => name,
        Err(error) => return fail_stage(report, driver, stage.name, error, Vec::new()),
    };
    if let Err(error) = waited {
        return fail_stage(report, driver, stage.name, error, vec![screenshot]);
    }
    for required in stage.required {
        if !observe::contains_line(&observed, required) {
            let error = format!("Expected the output line '{required}'.");
            return fail_stage(report, driver, stage.name, error, vec![screenshot]);
        }
    }
    report.stage(stage.name, true, stage.detail, &observed, vec![screenshot]);
    Ok(())
}

async fn run_tmux_attach(
    driver: &Driver,
    report: &mut report::Report,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    const NAME: &str = "tmux-attach";
    let stage = script::tmux_attach();
    let mut command = String::from(stage.command);
    command.push('\r');
    if let Err(error) = driver.send(command.as_bytes()) {
        return fail_stage(report, driver, NAME, error, Vec::new());
    }
    if let Err(error) =
        observe::wait_for_text(&driver.model, stage.sentinel, Duration::from_secs(10), cx).await
    {
        return fail_stage(report, driver, NAME, error, Vec::new());
    }
    if let Err(error) =
        observe::wait_for_alternate_screen(&driver.model, stage.fail, STAGE_TIMEOUT, cx).await
    {
        return fail_stage(report, driver, NAME, error, Vec::new());
    }
    // Record the mode as soon as tmux owns the screen, so a later failure
    // still detaches before cleaning up instead of typing into the pane.
    driver.set_mode(UiMode::Tmux);
    if let Err(error) = observe::wait_for_pane_markers(
        &driver.model,
        script::LEFT_MARKER,
        script::RIGHT_MARKER,
        stage.fail,
        Duration::from_secs(20),
        cx,
    )
    .await
    {
        return fail_stage(report, driver, NAME, error, Vec::new());
    }
    let first = match observe::capture(driver.handle, &driver.output, "03-tmux-split.png", cx) {
        Ok(name) => name,
        Err(error) => return fail_stage(report, driver, NAME, error, Vec::new()),
    };
    if let Err(error) = driver.send(script::TMUX_ZOOM) {
        return fail_stage(report, driver, NAME, error, vec![first]);
    }
    cx.background_executor().timer(MODE_SETTLE).await;
    if let Err(error) = driver.send(script::TMUX_REDRAW) {
        return fail_stage(report, driver, NAME, error, vec![first]);
    }
    cx.background_executor().timer(MODE_SETTLE).await;
    if let Err(error) = observe::wait_for_zoomed_pane(
        &driver.model,
        script::RIGHT_MARKER,
        script::LEFT_MARKER,
        Duration::from_secs(10),
        cx,
    )
    .await
    {
        return fail_stage(report, driver, NAME, error, vec![first]);
    }
    let zoomed = match observe::capture(driver.handle, &driver.output, "04-tmux-zoom.png", cx) {
        Ok(name) => name,
        Err(error) => return fail_stage(report, driver, NAME, error, vec![first]),
    };
    if let Err(error) = driver.send(script::TMUX_DETACH) {
        return fail_stage(report, driver, NAME, error, vec![first, zoomed]);
    }
    let waited = observe::wait_for_marker(
        &driver.model,
        stage.sentinel,
        stage.ok,
        stage.fail,
        Duration::from_secs(20),
        cx,
    )
    .await;
    if let Err(error) = waited {
        return fail_stage(report, driver, NAME, error, vec![first, zoomed]);
    }
    driver.set_mode(UiMode::Shell);
    if driver.model.modes().contains(Modes::ALT_SCREEN) {
        let error = "tmux detached but the alternate screen is still active.".to_string();
        return fail_stage(report, driver, NAME, error, vec![first, zoomed]);
    }
    let observed = observe::visible_text(&driver.model);
    let detached = match observe::capture(driver.handle, &driver.output, "05-tmux-detached.png", cx)
    {
        Ok(name) => name,
        Err(error) => {
            return fail_stage(report, driver, NAME, error, vec![first, zoomed]);
        }
    };
    report.stage(
        NAME,
        true,
        "tmux attached with two panes whose markers shared one row around the pane border, zoom showed only the active pane after a tmux client refresh, and detach returned to the shell.",
        &observed,
        vec![first, zoomed, detached],
    );
    Ok(())
}

async fn run_nvim(
    driver: &Driver,
    report: &mut report::Report,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    const NAME: &str = "nvim";
    let stage = script::nvim_launch();
    let mut command = String::from(stage.command);
    command.push('\r');
    if let Err(error) = driver.send(command.as_bytes()) {
        return fail_stage(report, driver, NAME, error, Vec::new());
    }
    if let Err(error) =
        observe::wait_for_text(&driver.model, stage.sentinel, Duration::from_secs(10), cx).await
    {
        return fail_stage(report, driver, NAME, error, Vec::new());
    }
    if let Err(error) =
        observe::wait_for_alternate_screen(&driver.model, stage.fail, STAGE_TIMEOUT, cx).await
    {
        return fail_stage(report, driver, NAME, error, Vec::new());
    }
    // Record the mode as soon as nvim owns the screen, so a later failure
    // still quits the editor before cleaning up.
    driver.set_mode(UiMode::Nvim);
    if let Err(error) = observe::wait_for_text(
        &driver.model,
        script::NVIM_ORIGINAL,
        Duration::from_secs(20),
        cx,
    )
    .await
    {
        return fail_stage(report, driver, NAME, error, Vec::new());
    }
    let buffer = match observe::capture(driver.handle, &driver.output, "06-nvim-buffer.png", cx) {
        Ok(name) => name,
        Err(error) => return fail_stage(report, driver, NAME, error, Vec::new()),
    };
    for step in script::NVIM_EDIT_STEPS {
        if let Err(error) = driver.send(step.bytes) {
            return fail_stage(report, driver, NAME, error, vec![buffer]);
        }
        cx.background_executor()
            .timer(Duration::from_millis(step.wait_ms))
            .await;
    }
    let edited = match observe::capture(driver.handle, &driver.output, "07-nvim-edited.png", cx) {
        Ok(name) => name,
        Err(error) => return fail_stage(report, driver, NAME, error, vec![buffer]),
    };
    if let Err(error) = driver.send(script::NVIM_QUIT) {
        return fail_stage(report, driver, NAME, error, vec![buffer, edited]);
    }
    let waited = observe::wait_for_marker(
        &driver.model,
        stage.sentinel,
        stage.ok,
        stage.fail,
        Duration::from_secs(20),
        cx,
    )
    .await;
    driver.set_mode(UiMode::Shell);
    if let Err(error) = waited {
        return fail_stage(report, driver, NAME, error, vec![buffer, edited]);
    }
    if driver.model.modes().contains(Modes::ALT_SCREEN) {
        let error = "nvim exited but the alternate screen is still active.".to_string();
        return fail_stage(report, driver, NAME, error, vec![buffer, edited]);
    }
    let observed = observe::visible_text(&driver.model);
    let exited = match observe::capture(driver.handle, &driver.output, "08-nvim-exited.png", cx) {
        Ok(name) => name,
        Err(error) => {
            return fail_stage(report, driver, NAME, error, vec![buffer, edited]);
        }
    };
    report.stage(
        NAME,
        true,
        "nvim entered the alternate screen, inserted text, undid and redid it, wrote and quit, and returned to the shell.",
        &observed,
        vec![buffer, edited, exited],
    );
    Ok(())
}

async fn cleanup(
    driver: &Driver,
    report: &mut report::Report,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    match driver.mode.get() {
        UiMode::Tmux => {
            let _ = driver.send(script::TMUX_DETACH);
            cx.background_executor().timer(MODE_SETTLE).await;
        }
        UiMode::Nvim => {
            let _ = driver.send(b"\x1b");
            let _ = driver.send(script::NVIM_FORCE_QUIT);
            cx.background_executor().timer(MODE_SETTLE).await;
        }
        UiMode::Shell => {}
    }
    if driver.model.modes().contains(Modes::ALT_SCREEN) {
        let _ = driver.send(b"\x03");
        cx.background_executor().timer(MODE_SETTLE).await;
    }
    driver.set_mode(UiMode::Shell);
    let stage = script::cleanup();
    let mut command = String::from(stage.command);
    command.push('\r');
    if let Err(error) = driver.send(command.as_bytes()) {
        let observed = observe::visible_text(&driver.model);
        report.stage("cleanup", false, &error, &observed, Vec::new());
        return Err(format!("stage 'cleanup' failed: {error}"));
    }
    let waited = observe::wait_for_marker(
        &driver.model,
        stage.sentinel,
        stage.ok,
        stage.fail,
        CLEANUP_TIMEOUT,
        cx,
    )
    .await;
    let observed = observe::visible_text(&driver.model);
    match waited {
        Ok(()) => {
            report.stage(
                "cleanup",
                true,
                "tmux was stopped and the private temporary HOME was removed.",
                &observed,
                Vec::new(),
            );
            Ok(())
        }
        Err(error) => {
            report.stage("cleanup", false, &error, &observed, Vec::new());
            Err(format!("stage 'cleanup' failed: {error}"))
        }
    }
}

async fn wait_for_connection(binding: &TabBinding, cx: &mut AsyncApp) -> Result<(), String> {
    let deadline = Instant::now() + CONNECTION_TIMEOUT;
    loop {
        let state = binding.scoped_state();
        if state.host_key_prompt().is_some() {
            return Err(
                "A host-key question appeared; the fixed profile must already be pinned."
                    .to_string(),
            );
        }
        match state.connection_state() {
            ConnectionState::Connected => return Ok(()),
            ConnectionState::Error => {
                let reason = state
                    .start_error()
                    .or_else(|| {
                        state
                            .connection_progress()
                            .map(|progress| progress.message.as_str())
                    })
                    .unwrap_or("no reason was reported");
                return Err(format!("The SSH connection failed: {reason}"));
            }
            ConnectionState::Disconnected => {
                return Err("The SSH connection closed before it became ready.".to_string());
            }
            ConnectionState::Connecting | ConnectionState::Reconnecting => {}
        }
        if Instant::now() >= deadline {
            let progress = state
                .connection_progress()
                .map(|progress| format!("{}: {}", progress.phase, progress.message))
                .unwrap_or_else(|| "no progress report".to_string());
            return Err(format!(
                "The SSH connection did not become ready within {} seconds ({progress}).",
                CONNECTION_TIMEOUT.as_secs()
            ));
        }
        cx.background_executor().timer(POLL_INTERVAL).await;
    }
}

fn active_tab(prototype: &Entity<Prototype>, cx: &App) -> Result<Rc<TabBinding>, String> {
    let window = prototype.read(cx);
    if window.tabs.len() != 1 {
        return Err(format!(
            "The field driver expects exactly one tab, but the window has {}.",
            window.tabs.len()
        ));
    }
    let tab = window.tabs.active();
    let binding = tab
        .binding
        .as_ref()
        .ok_or_else(|| "The active tab has no session binding.".to_string())?;
    if !binding.is_active() {
        return Err("The active tab's session binding is not marked active.".to_string());
    }
    Ok(Rc::clone(binding))
}

fn field_output() -> Result<PathBuf, String> {
    let path = field::output_dir()
        .or_else(|| std::env::var_os(OUTPUT_ENV).map(PathBuf::from))
        .ok_or_else(|| {
            format!("No field output directory was prepared; pass --output or set {OUTPUT_ENV}.")
        })?;
    validate_output(&path)?;
    Ok(path)
}

fn validate_output(path: &Path) -> Result<(), String> {
    if !path.is_dir() {
        return Err(format!(
            "The field output directory '{}' does not exist.",
            path.display()
        ));
    }
    let mut entries = std::fs::read_dir(path).map_err(|error| {
        format!(
            "The field output directory '{}' cannot be read: {error}",
            path.display()
        )
    })?;
    match entries.next() {
        None => Ok(()),
        Some(Ok(_)) => Err(format!(
            "The field output directory '{}' must be empty.",
            path.display()
        )),
        Some(Err(error)) => Err(format!(
            "The field output directory '{}' could not be listed: {error}",
            path.display()
        )),
    }
}

fn fail_start(handle: AnyWindowHandle, prototype: &Entity<Prototype>, cx: &mut App, error: &str) {
    eprintln!("tterm-gpui field: {error}");
    let _ = handle.update(cx, |_, window, app| {
        prototype.update(app, |prototype, cx| {
            prototype.request_close(window, cx);
        });
    });
}

fn fail_stage<T>(
    report: &mut report::Report,
    driver: &Driver,
    name: &str,
    error: String,
    screenshots: Vec<String>,
) -> Result<T, String> {
    let observed = observe::visible_text(&driver.model);
    report.stage(name, false, &error, &observed, screenshots);
    Err(format!("stage '{name}' failed: {error}"))
}
