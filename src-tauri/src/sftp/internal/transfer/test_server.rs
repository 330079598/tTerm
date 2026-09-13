//! In-process SSH + SFTP server used by the resumable-transfer tests.
//!
//! It speaks real SFTP over a real (duplex) SSH transport backed by a temp
//! directory, supports `limits@openssh.com`, random-offset read/write,
//! rename/remove, and can inject a per-request delay so the parallel
//! throughput benchmark is meaningful. It also lets tests disable
//! overwriting renames to exercise the remove-then-rename fallback.

use std::collections::HashMap;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::server::{Auth, Msg, Session};
use russh::{Channel, ChannelId};
use russh_sftp::extensions::LimitsExtension;
use russh_sftp::protocol::{
    Attrs, Data, ExtendedReply, File, FileAttributes, Handle, Name, OpenFlags, Packet, Status,
    StatusCode, Version,
};

use super::RemoteChannels;

#[derive(Clone, Debug)]
pub struct TestLimits {
    pub max_packet_len: u64,
    pub max_read_len: u64,
    pub max_write_len: u64,
}

impl TestLimits {
    fn to_extension(&self) -> LimitsExtension {
        LimitsExtension {
            max_packet_len: self.max_packet_len,
            max_read_len: self.max_read_len,
            max_write_len: self.max_write_len,
            max_open_handles: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ServerOptions {
    pub limits: Option<TestLimits>,
    pub latency: Duration,
    pub rename_overwrites: bool,
    /// Refuse every remove, so tests can exercise paths that must survive a
    /// discard whose removal keeps failing.
    pub fail_removes: bool,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            limits: None,
            latency: Duration::ZERO,
            rename_overwrites: true,
            fail_removes: false,
        }
    }
}

struct SshSession {
    root: PathBuf,
    options: Arc<ServerOptions>,
    channels: Arc<Mutex<HashMap<ChannelId, Channel<Msg>>>>,
}

impl russh::server::Handler for SshSession {
    type Error = russh::Error;

    async fn auth_password(&mut self, _user: &str, _password: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        self.channels
            .lock()
            .expect("channels lock")
            .insert(channel.id(), channel);
        Ok(true)
    }

    async fn subsystem_request(
        &mut self,
        channel_id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name == "sftp" {
            let channel = self
                .channels
                .lock()
                .expect("channels lock")
                .remove(&channel_id)
                .expect("channel for subsystem");
            let handler = SftpHandler {
                root: self.root.clone(),
                options: self.options.clone(),
                next_handle: 0,
                handles: HashMap::new(),
            };
            session.channel_success(channel_id)?;
            russh_sftp::server::run(channel.into_stream(), handler).await;
        } else {
            session.channel_failure(channel_id)?;
        }
        Ok(())
    }
}

enum HandleEntry {
    File(std::fs::File),
    Dir(Vec<File>),
}

struct SftpHandler {
    root: PathBuf,
    options: Arc<ServerOptions>,
    next_handle: u64,
    handles: HashMap<String, HandleEntry>,
}

impl SftpHandler {
    fn real_path(&self, path: &str) -> PathBuf {
        if path.is_empty() || path == "." || path == "/" {
            self.root.clone()
        } else {
            self.root.join(path.trim_start_matches('/'))
        }
    }

    fn ok(&self, id: u32) -> Status {
        Status {
            id,
            status_code: StatusCode::Ok,
            error_message: "Ok".to_string(),
            language_tag: "en".to_string(),
        }
    }

    fn attrs_for(path: &Path) -> Result<FileAttributes, StatusCode> {
        let metadata = std::fs::metadata(path).map_err(|_| StatusCode::NoSuchFile)?;
        Ok(FileAttributes::from(&metadata))
    }

    fn next_handle(&mut self, id: u32, entry: HandleEntry) -> Handle {
        self.next_handle += 1;
        let handle = format!("h{}", self.next_handle);
        self.handles.insert(handle.clone(), entry);
        Handle { id, handle }
    }
}

impl russh_sftp::server::Handler for SftpHandler {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        _version: u32,
        _extensions: HashMap<String, String>,
    ) -> Result<Version, Self::Error> {
        let mut version = Version::new();
        if self.options.limits.is_some() {
            version
                .extensions
                .insert("limits@openssh.com".to_string(), "1".to_string());
        }
        Ok(version)
    }

    async fn extended(
        &mut self,
        id: u32,
        request: String,
        _data: Vec<u8>,
    ) -> Result<Packet, Self::Error> {
        if request == "limits@openssh.com" {
            let limits = self
                .options
                .limits
                .as_ref()
                .ok_or(StatusCode::OpUnsupported)?;
            let bytes = russh_sftp::ser::to_bytes(&limits.to_extension())
                .map_err(|_| StatusCode::Failure)?;
            return Ok(Packet::ExtendedReply(ExtendedReply {
                id,
                data: bytes.to_vec(),
            }));
        }
        Err(StatusCode::OpUnsupported)
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let path = self.real_path(&filename);
        let options = std::fs::OpenOptions::from(pflags);
        let file = options.open(&path).map_err(|_| StatusCode::NoSuchFile)?;
        Ok(self.next_handle(id, HandleEntry::File(file)))
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        self.handles.remove(&handle);
        Ok(self.ok(id))
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        if !self.options.latency.is_zero() {
            tokio::time::sleep(self.options.latency).await;
        }
        let entry = self.handles.get_mut(&handle).ok_or(StatusCode::Failure)?;
        let HandleEntry::File(file) = entry else {
            return Err(StatusCode::Failure);
        };
        file.seek(std::io::SeekFrom::Start(offset))
            .map_err(|_| StatusCode::Failure)?;
        let mut buffer = vec![0u8; len as usize];
        let read = file.read(&mut buffer).map_err(|_| StatusCode::Failure)?;
        if read == 0 {
            return Err(StatusCode::Eof);
        }
        buffer.truncate(read);
        Ok(Data { id, data: buffer })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        if !self.options.latency.is_zero() {
            tokio::time::sleep(self.options.latency).await;
        }
        let entry = self.handles.get_mut(&handle).ok_or(StatusCode::Failure)?;
        let HandleEntry::File(file) = entry else {
            return Err(StatusCode::Failure);
        };
        file.seek(std::io::SeekFrom::Start(offset))
            .map_err(|_| StatusCode::Failure)?;
        file.write_all(&data).map_err(|_| StatusCode::Failure)?;
        Ok(self.ok(id))
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let attrs = Self::attrs_for(&self.real_path(&path))?;
        Ok(Attrs { id, attrs })
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        self.stat(id, path).await
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        let entry = self.handles.get(&handle).ok_or(StatusCode::Failure)?;
        match entry {
            HandleEntry::File(file) => {
                let metadata = file.metadata().map_err(|_| StatusCode::Failure)?;
                Ok(Attrs {
                    id,
                    attrs: FileAttributes::from(&metadata),
                })
            }
            HandleEntry::Dir(_) => Err(StatusCode::Failure),
        }
    }

    async fn setstat(
        &mut self,
        id: u32,
        _path: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        Ok(self.ok(id))
    }

    async fn fsetstat(
        &mut self,
        id: u32,
        _handle: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        Ok(self.ok(id))
    }

    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        std::fs::create_dir(self.real_path(&path)).map_err(|_| StatusCode::Failure)?;
        Ok(self.ok(id))
    }

    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        std::fs::remove_dir(self.real_path(&path)).map_err(|_| StatusCode::Failure)?;
        Ok(self.ok(id))
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
        if self.options.fail_removes {
            return Err(StatusCode::Failure);
        }
        std::fs::remove_file(self.real_path(&filename)).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                StatusCode::NoSuchFile
            } else {
                StatusCode::Failure
            }
        })?;
        Ok(self.ok(id))
    }

    async fn rename(
        &mut self,
        id: u32,
        oldpath: String,
        newpath: String,
    ) -> Result<Status, Self::Error> {
        let source = self.real_path(&oldpath);
        let target = self.real_path(&newpath);
        if !self.options.rename_overwrites && target.exists() {
            return Err(StatusCode::Failure);
        }
        std::fs::rename(source, target).map_err(|_| StatusCode::Failure)?;
        Ok(self.ok(id))
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        Ok(Name {
            id,
            files: vec![File::dummy(path)],
        })
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        let dir = std::fs::read_dir(self.real_path(&path)).map_err(|_| StatusCode::NoSuchFile)?;
        let mut files = Vec::new();
        for entry in dir {
            let entry = entry.map_err(|_| StatusCode::Failure)?;
            let metadata = entry.metadata().map_err(|_| StatusCode::Failure)?;
            files.push(File::new(
                entry.file_name().to_string_lossy().into_owned(),
                FileAttributes::from(&metadata),
            ));
        }
        Ok(self.next_handle(id, HandleEntry::Dir(files)))
    }

    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
        let entry = self.handles.get_mut(&handle).ok_or(StatusCode::Failure)?;
        let HandleEntry::Dir(files) = entry else {
            return Err(StatusCode::Failure);
        };
        if files.is_empty() {
            return Err(StatusCode::Eof);
        }
        let batch = std::mem::take(files);
        Ok(Name { id, files: batch })
    }
}

/// A running in-process SSH+SFTP server with a connected client.
pub struct TestServer {
    pub root: PathBuf,
    #[allow(dead_code)]
    server_task:
        tokio::task::JoinHandle<Result<russh::server::RunningSession<SshSession>, russh::Error>>,
    pub handle: russh::client::Handle<TestClient>,
    _server_dir: TempDir,
    client_dir: TempDir,
}

impl TestServer {
    pub async fn start(options: ServerOptions) -> Self {
        let server_dir = TempDir::new("tterm-sftp-server");
        let root = server_dir.path().to_path_buf();
        let client_dir = TempDir::new("tterm-sftp-client");
        let options = Arc::new(options);

        let (client_io, server_io) = tokio::io::duplex(8 * 1024 * 1024);

        let server_config = Arc::new(russh::server::Config {
            keys: vec![russh::keys::PrivateKey::random(
                &mut russh::keys::ssh_key::rand_core::OsRng,
                russh::keys::ssh_key::Algorithm::Ed25519,
            )
            .expect("host key")],
            ..Default::default()
        });

        let ssh_session = SshSession {
            root: root.clone(),
            options: options.clone(),
            channels: Arc::new(Mutex::new(HashMap::new())),
        };
        let server_task = tokio::spawn(async move {
            russh::server::run_stream(server_config, server_io, ssh_session).await
        });

        let client_config = Arc::new(russh::client::Config::default());
        let mut handle = russh::client::connect_stream(client_config, client_io, TestClient)
            .await
            .expect("connect client");

        let auth = handle
            .authenticate_password("user", "pass")
            .await
            .expect("authenticate");
        assert!(auth.success());

        Self {
            root,
            server_task,
            handle,
            _server_dir: server_dir,
            client_dir,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn local_dir(&self) -> &Path {
        self.client_dir.path()
    }

    /// Open `count` SFTP subsystem channels over the same SSH connection.
    pub async fn channels(&self, count: usize) -> RemoteChannels {
        let mut sessions = Vec::new();
        let mut limits = None;
        for _ in 0..count.max(1) {
            let (session, negotiated) = open_raw_session(&self.handle).await;
            if limits.is_none() {
                limits = negotiated;
            }
            sessions.push(session);
        }
        RemoteChannels::new(sessions, limits)
    }
}

pub struct TestClient;

impl russh::client::Handler for TestClient {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

async fn open_raw_session(
    handle: &russh::client::Handle<TestClient>,
) -> (
    Arc<russh_sftp::client::RawSftpSession>,
    Option<LimitsExtension>,
) {
    let channel = handle.channel_open_session().await.expect("open channel");
    channel
        .request_subsystem(true, "sftp")
        .await
        .expect("request sftp subsystem");
    let mut session = russh_sftp::client::RawSftpSession::new(channel.into_stream());
    let version = session.init().await.expect("sftp init");
    let limits = if version
        .extensions
        .get("limits@openssh.com")
        .is_some_and(|value| value == "1")
    {
        let extension = session.limits().await.expect("limits");
        session.set_limits(Arc::new(russh_sftp::client::rawsession::Limits {
            read_len: (extension.max_read_len > 0).then_some(extension.max_read_len),
            write_len: (extension.max_write_len > 0).then_some(extension.max_write_len),
            open_handles: None,
        }));
        Some(extension)
    } else {
        None
    };
    (Arc::new(session), limits)
}

/// Minimal self-cleaning temp directory (avoids an extra direct dependency).
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!("{prefix}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
