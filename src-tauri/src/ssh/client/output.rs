/// Runs a call that may block on the batcher's bounded queue (send, or the
/// join inside finish) off the async worker: `block_in_place` on a
/// multi-thread runtime keeps the connection's select! loop responsive.
/// `block_in_place` panics outside a multi-thread runtime, so fall back to
/// running inline on a current-thread runtime or plain thread (the sender
/// runs on its own OS thread, so this cannot deadlock).
pub(super) fn run_blocking<T>(f: impl FnOnce() -> T) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(f)
        }
        _ => f(),
    }
}

/// Forwards one output batch to the webview, converted to UTF-8 when the
/// session uses another charset.
pub(super) fn deliver_output(
    app: &tauri::AppHandle,
    tab_id: &str,
    sender: Option<&crate::terminal::io_batcher::TerminalOutputSender>,
    decoder: Option<&mut crate::terminal::encoding::OutputDecoder>,
    payload: Vec<u8>,
) {
    let payload = match decoder {
        Some(decoder) => {
            let decoded = decoder.decode(&payload);
            // Logged after decoding (see `record_raw_output`).
            crate::session_log::record_output(app, tab_id, &decoded);
            decoded
        }
        None => payload,
    };
    if payload.is_empty() {
        return;
    }
    match sender {
        Some(sender) => run_blocking(|| sender.send(payload)),
        None => {
            emit_pty_output(app, tab_id, String::from_utf8_lossy(&payload).into_owned());
        }
    }
}

fn emit_pty_output(app: &tauri::AppHandle, tab_id: &str, payload: String) {
    crate::terminal::legacy_events::emit_output(app, tab_id, payload);
}
