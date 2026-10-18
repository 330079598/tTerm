//! Reads which Linux distribution an SSH host runs, so its tab can show the
//! distribution's icon.

use crate::monitor::metrics::parse_os_release;
use crate::monitor::types::LinuxDistributionInfo;
use russh::client::{Handle, Handler, Msg};
use russh::{Channel, ChannelMsg};
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Reports what a host runs; `distribution` is null for a host without
/// `/etc/os-release`, such as macOS, a BSD or a network device.
const REMOTE_OS_EVENT: &str = "ssh-remote-os";

/// Bounds opening the channel and, separately, reading its output, so a
/// server that never answers costs the terminal nothing.
const TIMEOUT: Duration = Duration::from_secs(3);

const COMMAND: &str = "cat /etc/os-release 2>/dev/null";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteOsPayload {
    tab_id: String,
    distribution: Option<LinuxDistributionInfo>,
}

/// Asks the host for its `/etc/os-release` on a channel of its own, and
/// reports the answer for `tab_id` in the background. A server that refuses
/// another channel, such as one allowing a single session, reports nothing.
pub(crate) async fn probe<H: Handler>(app: &AppHandle, tab_id: &str, session: &Handle<H>) {
    let opened = tokio::time::timeout(TIMEOUT, async {
        let channel = session.channel_open_session().await.ok()?;
        channel.exec(true, COMMAND).await.ok()?;
        Some(channel)
    })
    .await;
    let Ok(Some(channel)) = opened else {
        return;
    };
    let app = app.clone();
    let tab_id = tab_id.to_string();
    tokio::spawn(async move {
        let Ok(output) = tokio::time::timeout(TIMEOUT, read_output(channel)).await else {
            return;
        };
        let payload = RemoteOsPayload {
            tab_id,
            distribution: parse_os_release(&output),
        };
        if let Err(error) = app.emit(REMOTE_OS_EVENT, payload) {
            eprintln!("Failed to report the remote OS: {error}");
        }
    });
}

/// Everything the command prints, once it is done.
async fn read_output(mut channel: Channel<Msg>) -> String {
    let mut output = Vec::new();
    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Data { data } => output.extend_from_slice(&data),
            ChannelMsg::Failure | ChannelMsg::Eof | ChannelMsg::Close => break,
            _ => {}
        }
    }
    let _ = channel.close().await;
    String::from_utf8_lossy(&output).into_owned()
}
