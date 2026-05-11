use russh::client;
use russh_sftp::client::SftpSession;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::watch;
use tokio::sync::RwLock;

use crate::ssh::{JumpChain, SshClientHandler};

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
    /// Jump host chain kept alive to maintain every tunnel channel.
    /// `None` for direct connections.
    pub jump_chain: Option<JumpChain>,
    pub ssh: client::Handle<SshClientHandler>,
    pub sftp: Arc<SftpSession>,
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
    pub jump_chain: Option<String>,
}

impl SftpConnectionKey {
    pub fn from_plan(
        tab_id: &str,
        plan: &crate::core::session::SessionPlan,
    ) -> Result<Self, String> {
        Ok(Self {
            tab_id: tab_id.to_string(),
            host: plan.host.clone().ok_or("Host is required")?,
            port: plan.port,
            username: plan.username.clone().ok_or("Username is required")?,
            jump_chain: if plan.jump_hosts.is_empty() {
                None
            } else {
                Some(
                    plan.jump_hosts
                        .iter()
                        .map(|jump| {
                            format!(
                                "{}:{}:{}:{}",
                                jump.host,
                                jump.port,
                                jump.username,
                                if jump.private_key_path.is_some() {
                                    "key"
                                } else {
                                    "password"
                                }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("|"),
                )
            },
        })
    }
}

pub type SftpConnectionPool = Arc<RwLock<HashMap<SftpConnectionKey, CachedSftpConnection>>>;
