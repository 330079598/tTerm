use crate::core::session::PtyConnectionOptions;
use crate::core::state::HostPromptMap;
use crate::sftp::internal::connection::{ensure_ssh_plan, map_sftp_error};
use crate::sftp::internal::types::SftpConnectionPool;
use crate::ssh::SecretStoreState;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::FileAttributes;
use tauri::{AppHandle, State};

/// The bits a chmod may set: rwx for owner, group and others, plus setuid,
/// setgid and sticky. The file type bits are not the client's to change.
pub(crate) const PERMISSION_MASK: u32 = 0o7777;

/// Sets the permission bits of each path, stopping at the first failure.
/// Like `chmod`, a symlink's target is changed, not the link.
pub(crate) async fn set_permissions(
    sftp: &SftpSession,
    paths: &[String],
    mode: u32,
) -> Result<(), String> {
    if mode > PERMISSION_MASK {
        return Err(format!("Invalid permission mode {mode:o}"));
    }
    for path in paths {
        let attributes = FileAttributes {
            permissions: Some(mode),
            ..FileAttributes::empty()
        };
        sftp.set_metadata(path.as_str(), attributes)
            .await
            .map_err(|err| format!("{path}: {}", map_sftp_error(err)))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn sftp_set_permissions(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    paths: Vec<String>,
    mode: u32,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        set_permissions(sftp, &paths, mode).await
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::set_permissions;
    use crate::sftp::internal::transfer::test_server::{ServerOptions, TestServer};
    use std::os::unix::fs::PermissionsExt;

    fn mode_of(path: &std::path::Path) -> u32 {
        std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o7777
    }

    #[tokio::test]
    async fn changes_the_mode_of_every_path() {
        let server = TestServer::start(ServerOptions::default()).await;
        let script = server.root().join("deploy.sh");
        let folder = server.root().join("logs");
        std::fs::write(&script, "#!/bin/sh\n").expect("write file");
        std::fs::create_dir(&folder).expect("create folder");
        let sftp = server.sftp_session().await;

        set_permissions(
            &sftp,
            &["/deploy.sh".to_string(), "/logs".to_string()],
            0o750,
        )
        .await
        .expect("set permissions");

        assert_eq!(mode_of(&script), 0o750);
        assert_eq!(mode_of(&folder), 0o750);
    }

    /// The same call against OpenSSH's own `sftp-server`, which is what the
    /// command talks to in practice.
    #[tokio::test]
    #[ignore = "spawns a real sshd; run with --ignored real_sshd"]
    async fn real_sshd_applies_the_mode() {
        use crate::ssh::auth::{authenticate, AuthPrompter, AuthTarget};
        use crate::ssh::test_sshd::{current_user, TempDir, TestSshd};

        let sshd =
            TestSshd::spawn(12321, &["-t", "ed25519"], "Subsystem sftp internal-sftp\n").await;
        let username = current_user();
        let mut session = sshd.connect().await;
        authenticate(
            &mut session,
            &AuthTarget {
                host: "127.0.0.1",
                port: sshd.port,
                username: &username,
                hop: None,
            },
            sshd.key_method(),
            &AuthPrompter::Unavailable,
        )
        .await
        .expect("authenticate");
        let channel = session.channel_open_session().await.expect("open channel");
        channel
            .request_subsystem(true, "sftp")
            .await
            .expect("sftp subsystem");
        let sftp = russh_sftp::client::SftpSession::new(channel.into_stream())
            .await
            .expect("sftp session");

        let dir = TempDir::new();
        let file = dir.path.join("report.txt");
        std::fs::write(&file, "data").expect("write file");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600))
            .expect("initial mode");

        set_permissions(&sftp, &[file.to_string_lossy().into_owned()], 0o644)
            .await
            .expect("set permissions");
        assert_eq!(mode_of(&file), 0o644);
    }

    #[tokio::test]
    async fn rejects_file_type_bits_and_names_the_failing_path() {
        let server = TestServer::start(ServerOptions::default()).await;
        let sftp = server.sftp_session().await;

        let error = set_permissions(&sftp, &["/missing".to_string()], 0o100644)
            .await
            .expect_err("type bits");
        assert_eq!(error, "Invalid permission mode 100644");

        let error = set_permissions(&sftp, &["/missing".to_string()], 0o644)
            .await
            .expect_err("missing path");
        assert!(error.starts_with("/missing: "), "{error}");
    }
}
