//! Wraps Tauri binary IPC as a session byte sender.
use std::sync::Arc;
use tauri::ipc::{Channel, InvokeResponseBody};
use tterm_core::terminal::io_batcher::OutputFlow;
use tterm_core::terminal::output::TerminalOutput;

pub(crate) fn new(channel: Channel<InvokeResponseBody>) -> TerminalOutput {
    with_flow(channel, Arc::new(OutputFlow::default()))
}

pub(crate) fn with_flow(
    channel: Channel<InvokeResponseBody>,
    flow: Arc<OutputFlow>,
) -> TerminalOutput {
    TerminalOutput::new(
        Arc::new(move |bytes| {
            channel
                .send(InvokeResponseBody::Raw(bytes))
                .map_err(|error| error.to_string())
        }),
        flow,
    )
}
