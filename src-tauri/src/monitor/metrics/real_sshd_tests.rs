//! The metrics script against a real OpenSSH server and the host's own
//! tools (see [`crate::ssh::test_sshd`]), which shows the script's section
//! for the system the tests run on, Linux or macOS, working end to end.

use super::{parse_metrics_output, METRICS_SCRIPT};
use crate::sftp::internal::exec::RemoteScript;
use crate::ssh::test_sshd::TestSshd;

const SFTP: &str = "Subsystem sftp internal-sftp\n";

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_collects_metrics_for_this_system() {
    let sshd = TestSshd::spawn(12351, &["-t", "ed25519"], SFTP).await;
    let (ssh, _sftp) = sshd.sftp_session().await;

    let mut script = RemoteScript::start(&ssh, METRICS_SCRIPT)
        .await
        .expect("start script");
    let mut stdout = Vec::new();
    while let Some(output) = script.next_output().await {
        stdout.extend_from_slice(&output);
    }
    let outcome = script.finish().await;
    assert!(
        outcome.succeeded(),
        "{}",
        outcome.failure_message("metrics")
    );
    let output = String::from_utf8_lossy(&stdout);
    let snapshot = parse_metrics_output(&output);

    assert!(snapshot.supported, "{output}");
    assert!(snapshot.distribution.is_some(), "{output}");
    let memory = snapshot.memory.as_ref().expect("memory");
    assert!(memory.total_kib > 0 && memory.used_kib <= memory.total_kib);
    assert!(snapshot.load_average.is_some(), "{output}");
    assert!(
        snapshot.uptime_secs.is_some_and(|secs| secs > 0),
        "{output}"
    );
    assert!(snapshot.disk.is_some(), "{output}");
    assert!(snapshot.network.is_some(), "{output}");
    if cfg!(target_os = "macos") {
        let cpu = snapshot.cpu_percent.expect("cpu percent");
        assert!((0.0..=100.0).contains(&cpu));
    } else {
        assert!(snapshot.cpu_times.is_some(), "{output}");
        assert!(!snapshot.cpu_core_times.is_empty(), "{output}");
    }
}
