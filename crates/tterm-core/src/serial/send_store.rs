//! The serial send bar's per-connection keys and its hidden-echo list, from
//! `src/lib/serialSendStore.ts`.
//!
//! The list lives in the WebView's `localStorage` until P1-10 moves it; these
//! functions take the stored text and return the value to persist.

use serde_json::Value;

/// Connections that hide what their send bar sends. Showing it is the
/// default, and a per-device view choice, so it stays in this browser while
/// the history goes to the database.
pub const SERIAL_SEND_ECHO_STORAGE_KEY: &str = "tterm.serial.sendEcho";

/// Connections remembered; the least recently changed is forgotten first.
const CONNECTION_LIMIT: usize = 50;

/// The key a connection's history is kept under: its saved profile, else its
/// port.
pub fn serial_send_key(profile_id: Option<&str>, port: &str) -> String {
    match profile_id {
        Some(profile_id) => format!("profile:{profile_id}"),
        None => format!("port:{port}"),
    }
}

/// Whether the connection's send bar shows what it sends in the terminal.
pub fn load_serial_send_echo(key: &str, hidden: &[String]) -> bool {
    !hidden.iter().any(|item| item == key)
}

/// Records whether `key` hides its echo, returning the list to persist.
pub fn save_serial_send_echo(hidden: &[String], key: &str, echo: bool) -> Vec<String> {
    let mut next: Vec<String> = hidden
        .iter()
        .filter(|item| item.as_str() != key)
        .cloned()
        .collect();
    if echo {
        return next;
    }
    next.push(key.to_owned());
    if next.len() > CONNECTION_LIMIT {
        next.drain(..next.len() - CONNECTION_LIMIT);
    }
    next
}

/// Reads the stored list, ignoring anything that is not a list of strings.
pub fn parse_serial_send_echo(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        load_serial_send_echo, parse_serial_send_echo, save_serial_send_echo, serial_send_key,
    };

    #[test]
    fn keys_saved_connections_by_profile_and_others_by_port() {
        assert_eq!(serial_send_key(Some("board"), "COM3"), "profile:board");
        assert_eq!(serial_send_key(None, "COM3"), "port:COM3");
    }

    #[test]
    fn shows_by_default_and_remembers_a_connection_that_hides() {
        let key = serial_send_key(Some("board"), "COM3");
        let mut hidden = Vec::new();
        assert!(load_serial_send_echo(&key, &hidden));
        hidden = save_serial_send_echo(&hidden, &key, false);
        assert!(!load_serial_send_echo(&key, &hidden));
        assert!(load_serial_send_echo("port:COM3", &hidden));
        hidden = save_serial_send_echo(&hidden, &key, true);
        assert!(load_serial_send_echo(&key, &hidden));
    }

    #[test]
    fn ignores_what_it_cant_read() {
        assert!(load_serial_send_echo(
            "port:COM1",
            &parse_serial_send_echo(Some("not json"))
        ));
        assert!(!load_serial_send_echo(
            "port:COM1",
            &parse_serial_send_echo(Some(r#"[1,"port:COM1"]"#))
        ));
    }

    #[test]
    fn forgets_the_least_recently_hidden_connection_first() {
        let mut hidden = Vec::new();
        for index in 0..51 {
            hidden = save_serial_send_echo(&hidden, &format!("port:COM{index}"), false);
        }
        assert!(load_serial_send_echo("port:COM0", &hidden));
        assert!(!load_serial_send_echo("port:COM50", &hidden));
    }
}
