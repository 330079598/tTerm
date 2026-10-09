//! Toast notifications. A toast's Activated event fires while tTerm runs, so
//! no COM activator is registered; clicking one after tTerm quit starts it
//! through its Start menu shortcut instead.

use std::collections::VecDeque;
use std::sync::Mutex;

use tauri::AppHandle;
use windows::core::HSTRING;
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

use super::NotificationRequest;

/// Toasts still on screen or in the Action Center keep their click handler
/// only while referenced; the oldest are let go.
const KEPT_TOASTS: usize = 32;
static TOASTS: Mutex<VecDeque<ToastNotification>> = Mutex::new(VecDeque::new());

/// PowerShell's AppUserModelID, which notify-rust and Tauri's notification
/// plugin borrow too when no shortcut names the app.
const POWERSHELL_APP_ID: &str =
    "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe";

/// The installer's Start menu shortcut carries the bundle identifier as its
/// AppUserModelID. A build run from `target/` has no shortcut, and toasts for
/// an unknown ID are dropped silently.
fn app_user_model_id(app: &AppHandle) -> String {
    let from_target_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()))
        .is_some_and(|dir| {
            let profile = dir.file_name().and_then(|name| name.to_str());
            let parent = dir
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str());
            matches!(profile, Some("debug" | "release")) && parent == Some("target")
        });
    if from_target_dir {
        POWERSHELL_APP_ID.to_string()
    } else {
        app.config().identifier.clone()
    }
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn toast_xml(request: &NotificationRequest) -> String {
    let mut lines = format!("<text>{}</text>", escape_xml(&request.title));
    if let Some(subtitle) = request.subtitle.as_deref().filter(|text| !text.is_empty()) {
        lines.push_str(&format!("<text>{}</text>", escape_xml(subtitle)));
    }
    lines.push_str(&format!("<text>{}</text>", escape_xml(&request.body)));
    let audio = if request.sound {
        ""
    } else {
        r#"<audio silent="true"/>"#
    };
    format!(
        r#"<toast><visual><binding template="ToastGeneric">{lines}</binding></visual>{audio}</toast>"#
    )
}

pub fn show(app: &AppHandle, request: NotificationRequest) -> Result<(), String> {
    let show = || -> windows::core::Result<()> {
        let document = XmlDocument::new()?;
        document.LoadXml(&HSTRING::from(toast_xml(&request)))?;
        let toast = ToastNotification::CreateToastNotification(&document)?;
        let app_handle = app.clone();
        let tab_id = request.tab_id.clone();
        toast.Activated(&TypedEventHandler::new(move |_, _| {
            super::activated(&app_handle, tab_id.clone());
            Ok(())
        }))?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_user_model_id(
            app,
        )))?
        .Show(&toast)?;

        if let Ok(mut toasts) = TOASTS.lock() {
            if toasts.len() >= KEPT_TOASTS {
                toasts.pop_front();
            }
            toasts.push_back(toast);
        }
        Ok(())
    };
    show().map_err(|error| format!("Failed to show a notification: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_xml_escapes_text_and_silences_on_request() {
        let xml = toast_xml(&NotificationRequest {
            title: "a <b> & c".into(),
            subtitle: None,
            body: "\"done\"".into(),
            tab_id: None,
            sound: false,
        });
        assert!(xml.contains("<text>a &lt;b&gt; &amp; c</text><text>&quot;done&quot;</text>"));
        assert!(xml.contains(r#"<audio silent="true"/>"#));
    }
}
