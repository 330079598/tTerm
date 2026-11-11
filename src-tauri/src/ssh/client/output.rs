use crate::session_log::LogContext;
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

pub(super) struct OutputEndpoint<'a> {
    pub(super) sender: Option<&'a crate::terminal::io_batcher::TerminalOutputSender>,
    pub(super) sink: &'a dyn tterm_core::session::sink::SessionSink,
}

/// Forwards one output batch to the webview, converted to UTF-8 when the
/// session uses another charset.
pub(super) fn deliver_output(
    app: &LogContext,
    tab_id: &str,
    endpoint: OutputEndpoint<'_>,
    decoder: Option<&mut crate::terminal::encoding::OutputDecoder>,
    payload: Vec<u8>,
) {
    let OutputEndpoint { sender, sink } = endpoint;
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
            let _ = sink.emit(tterm_core::events::AppEvent::Terminal(
                tterm_core::events::terminal::TerminalEvent::Output {
                    tab_id: tab_id.to_string(),
                    text: String::from_utf8_lossy(&payload).into_owned(),
                },
            ));
        }
    }
}
