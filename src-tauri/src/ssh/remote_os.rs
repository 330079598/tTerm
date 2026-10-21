//! Reads which system an SSH host runs, so its tab can show the
//! distribution's icon.

use crate::monitor::metrics::parse_os_identity;
use crate::monitor::types::LinuxDistributionInfo;
use crate::sftp::internal::exec::RemoteScript;
use russh::client::{Handle, Handler};
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Reports what a host runs; `distribution` is null for a host that says
/// nothing about it, such as OpenBSD or a network device.
const REMOTE_OS_EVENT: &str = "ssh-remote-os";

/// Bounds starting the script and, separately, reading its output, so a
/// server that never answers costs the terminal nothing.
const TIMEOUT: Duration = Duration::from_secs(3);

/// Linux and FreeBSD describe themselves in `/etc/os-release`, macOS through
/// `sw_vers`. It runs under `sh -s`, so a csh login shell, which would read
/// `2>/dev/null` as an argument, does not matter.
const SCRIPT: &str = "if [ -r /etc/os-release ]; then cat /etc/os-release; \
                      elif command -v sw_vers >/dev/null 2>&1; then sw_vers; fi\n";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteOsPayload {
    tab_id: String,
    distribution: Option<LinuxDistributionInfo>,
}

/// Asks the host what it runs on a channel of its own, and reports the
/// answer for `tab_id` in the background. A server that refuses another
/// channel, such as one allowing a single session, reports nothing.
pub(crate) async fn probe<H: Handler>(app: &AppHandle, tab_id: &str, session: &Handle<H>) {
    let Ok(Ok(script)) = tokio::time::timeout(TIMEOUT, RemoteScript::start(session, SCRIPT)).await
    else {
        return;
    };
    let app = app.clone();
    let tab_id = tab_id.to_string();
    tokio::spawn(async move {
        let Ok(output) = tokio::time::timeout(TIMEOUT, read_output(script)).await else {
            return;
        };
        let payload = RemoteOsPayload {
            tab_id,
            distribution: parse_os_identity(&output),
        };
        if let Err(error) = app.emit(REMOTE_OS_EVENT, payload) {
            eprintln!("Failed to report the remote OS: {error}");
        }
    });
}

/// Everything the script prints, once it is done.
async fn read_output(mut script: RemoteScript) -> String {
    let mut output = Vec::new();
    while let Some(chunk) = script.next_output().await {
        output.extend_from_slice(&chunk);
    }
    script.finish().await;
    String::from_utf8_lossy(&output).into_owned()
}

#[cfg(all(test, unix))]
mod tests {
    use super::{read_output, SCRIPT};
    use crate::monitor::metrics::parse_os_identity;
    use crate::sftp::internal::exec::RemoteScript;
    use crate::ssh::test_sshd::TestSshd;

    #[tokio::test]
    #[ignore = "spawns a real sshd; run with --ignored real_sshd"]
    async fn real_sshd_names_this_system() {
        let sshd =
            TestSshd::spawn(12352, &["-t", "ed25519"], "Subsystem sftp internal-sftp\n").await;
        let (ssh, _sftp) = sshd.sftp_session().await;
        let script = RemoteScript::start(&ssh, SCRIPT)
            .await
            .expect("start script");
        let output = read_output(script).await;

        let distribution = parse_os_identity(&output).expect("distribution");
        if cfg!(target_os = "macos") {
            assert_eq!(distribution.id, "macos");
        } else {
            assert!(!distribution.id.is_empty(), "{output}");
        }
    }
}
