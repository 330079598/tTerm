use russh::client;
use russh_sftp::client::SftpSession;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::watch;
use tokio::sync::RwLock;

use crate::ssh::SshClientHandler;

pub type CancelSender = watch::Sender<bool>;
pub type TransferCancelMap = Arc<RwLock<HashMap<String, CancelSender>>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpDirectoryEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: Option<u64>,
    pub modified_at: Option<i64>,
    pub permissions: Option<String>,
    pub owner: Option<String>,
    pub group: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpDirectoryListing {
    pub current_path: String,
    pub parent_path: Option<String>,
    pub entries: Vec<SftpDirectoryEntry>,
}

pub struct ConnectedSftp {
    pub ssh: client::Handle<SshClientHandler>,
    pub sftp: SftpSession,
}

pub struct CachedSftpConnection {
    pub connection: ConnectedSftp,
    pub last_used: Instant,
}

#[derive(Hash, Eq, PartialEq, Clone)]
pub struct SftpConnectionKey {
    pub tab_id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
}

pub type SftpConnectionPool = Arc<RwLock<HashMap<SftpConnectionKey, CachedSftpConnection>>>;