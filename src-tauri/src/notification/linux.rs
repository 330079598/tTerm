//! Notifications through `notify-send` (libnotify). Since libnotify 0.7.9 it
//! can wait for the notification's default action and print it, which is
//! how a click is noticed; older versions show it without one.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tauri::AppHandle;

use super::NotificationRequest;

/// How long to wait for a click before letting `notify-send` go.
const CLICK_WAIT: Duration = Duration::from_secs(30 * 60);
const DEFAULT_ACTION: &str = "default";

pub fn show(app: &AppHandle, request: NotificationRequest) -> Result<(), String> {
    // Fails here rather than silently in the thread when libnotify is missing.
    Command::new("notify-send")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| {
            format!("notify-send is not available ({error}); install libnotify, e.g. libnotify-bin")
        })?;

    let summary = request.title.clone();
    let body = match request.subtitle.as_deref().filter(|text| !text.is_empty()) {
        Some(subtitle) => format!("{subtitle}\n{}", request.body),
        None => request.body.clone(),
    };
    let app = app.clone();
    std::thread::spawn(move || {
        let clickable = Command::new("notify-send")
            .arg("--app-name=tTerm")
            .arg(format!("--action={DEFAULT_ACTION}=Open"))
            .arg("--wait")
            .arg("--")
            .arg(&summary)
            .arg(&body)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match clickable {
            Ok(child) => child,
            Err(error) => {
                eprintln!("Failed to run notify-send: {error}");
                return;
            }
        };

        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if started.elapsed() < CLICK_WAIT => {
                    std::thread::sleep(Duration::from_millis(500))
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
            }
        };

        match status {
            Some(status) if status.success() => {
                let mut output = String::new();
                if let Some(mut stdout) = child.stdout.take() {
                    let _ = std::io::Read::read_to_string(&mut stdout, &mut output);
                }
                if output.trim() == DEFAULT_ACTION {
                    super::activated(&app, request.tab_id.clone());
                }
            }
            // An older notify-send that knows no actions.
            Some(_) => {
                let _ = Command::new("notify-send")
                    .arg("--app-name=tTerm")
                    .arg("--")
                    .arg(&summary)
                    .arg(&body)
                    .status();
            }
            None => {}
        }
    });
    Ok(())
}
