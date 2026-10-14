//! The count on tTerm's launcher in docks that follow Unity's LauncherEntry
//! DBus API: Ubuntu Dock, Dash to Panel, KDE Plasma's task manager, Plank.
//! Tauri sends it through libunity, and only while the Unity desktop itself
//! runs, so it never reached those docks; the signal is sent here instead.

use std::collections::HashMap;

use tauri::AppHandle;
use tokio::sync::OnceCell;
use zbus::names::BusName;
use zbus::zvariant::Value;
use zbus::Connection;

const INTERFACE: &str = "com.canonical.Unity.LauncherEntry";
/// Docks find the app from the signal's first argument; any path will do.
const OBJECT_PATH: &str = "/com/canonical/unity/launcherentry/tTerm";

/// Kept open while tTerm runs: docks drop a count once the connection that
/// sent it closes.
static CONNECTION: OnceCell<Connection> = OnceCell::const_new();

/// Shows `count` on the launcher whose desktop file is tTerm's, or hides it
/// at zero.
pub async fn set_count(app: &AppHandle, count: u32) -> Result<(), String> {
    let connection = CONNECTION
        .get_or_try_init(Connection::session)
        .await
        .map_err(|error| format!("Failed to reach the session bus to show the count: {error}"))?;
    // The desktop file the deb and rpm packages install, as Tauri names it.
    let app_uri = format!("application://{}.desktop", app.package_info().name);
    connection
        .emit_signal(
            None::<BusName<'_>>,
            OBJECT_PATH,
            INTERFACE,
            "Update",
            &(app_uri, properties(count)),
        )
        .await
        .map_err(|error| format!("Failed to show the notification count: {error}"))
}

fn properties(count: u32) -> HashMap<&'static str, Value<'static>> {
    HashMap::from([
        ("count", Value::from(i64::from(count))),
        ("count-visible", Value::from(count > 0)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_hides_the_count() {
        assert_eq!(properties(0)["count-visible"], Value::from(false));
        assert_eq!(properties(3)["count"], Value::from(3_i64));
        assert_eq!(properties(3)["count-visible"], Value::from(true));
    }
}
