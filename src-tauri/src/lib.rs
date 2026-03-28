use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use russh::client;
use russh::keys::ssh_key::HashAlg;
use russh::ChannelMsg;
use russh::Disconnect;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, oneshot, watch, Mutex as TokioMutex, RwLock};

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AppConfig {
    theme: String,
    language: String,
    #[serde(default = "default_font_family")]
    font_family: String,
    #[serde(default = "default_font_size")]
    font_size: u16,
}

fn default_font_family() -> String {
    #[cfg(target_os = "macos")]
    return "Menlo, Monaco, monospace".to_string();
    #[cfg(target_os = "windows")]
    return "\"Cascadia Code\", Consolas, monospace".to_string();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return "\"DejaVu Sans Mono\", monospace".to_string();
}

fn default_font_size() -> u16 {
    14
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
            language: "en".to_string(),
            font_family: default_font_family(),
            font_size: default_font_size(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SessionData {
    tabs: serde_json::Value,
    active_tab_id: Option<String>,
    last_saved: i64,
}

impl Default for SessionData {
    fn default() -> Self {
        Self {
            tabs: serde_json::json!([]),
            active_tab_id: None,
            last_saved: 0,
        }
    }
}

fn get_config_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "Failed to get HOME directory")?;
    let config_dir = PathBuf::from(home).join(".config").join("tterm");
    Ok(config_dir)
}

fn ensure_config_dir() -> Result<PathBuf, String> {
    let config_dir = get_config_path()?;
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    Ok(config_dir)
}

#[tauri::command]
fn load_config() -> Result<AppConfig, String> {
    let config_dir = get_config_path()?;
    let config_file = config_dir.join("config.json");
    if !config_file.exists() {
        return Ok(AppConfig::default());
    }
    let content = fs::read_to_string(&config_file)
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))
}

#[tauri::command]
fn save_config(config: AppConfig) -> Result<(), String> {
    let config_dir = ensure_config_dir()?;
    let config_file = config_dir.join("config.json");
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    fs::write(&config_file, content).map_err(|e| format!("Failed to write config file: {}", e))
}

#[tauri::command]
fn load_session() -> Result<SessionData, String> {
    let config_dir = get_config_path()?;
    let session_file = config_dir.join("session.json");
    if !session_file.exists() {
        return Ok(SessionData::default());
    }
    let content = fs::read_to_string(&session_file)
        .map_err(|e| format!("Failed to read session file: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse session: {}", e))
}

#[tauri::command]
fn save_session(session: SessionData) -> Result<(), String> {
    let config_dir = ensure_config_dir()?;
    let session_file = config_dir.join("session.json");
    let content = serde_json::to_string_pretty(&session)
        .map_err(|e| format!("Failed to serialize session: {}", e))?;
    fs::write(&session_file, content).map_err(|e| format!("Failed to write session file: {}", e))
}

#[tauri::command]
fn clear_session() -> Result<(), String> {
    let config_dir = get_config_path()?;
    let session_file = config_dir.join("session.json");
    if session_file.exists() {
        fs::remove_file(&session_file)
            .map_err(|e| format!("Failed to remove session file: {}", e))?;
    }
    Ok(())
}

// ── Saved Profiles ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SavedProfile {
    id: String,
    name: String,
    #[serde(default)]
    group: String,
    connection_type: String,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    #[serde(default)]
    remember_password: bool,
    auth_method: Option<String>,
    private_key_path: Option<String>,
    private_key_passphrase: Option<String>,
    #[serde(default)]
    reconnect: bool,
    #[serde(default = "default_reconnect_delay")]
    reconnect_delay_secs: u32,
    #[serde(default = "default_reconnect_max_delay")]
    reconnect_max_delay_secs: u32,
    #[serde(default = "default_reconnect_max_retries")]
    reconnect_max_retries: u32,
    #[serde(default = "default_keepalive_interval")]
    keepalive_interval_secs: u32,
    #[serde(default = "default_keepalive_count")]
    keepalive_count_max: u32,
}

fn default_reconnect_delay() -> u32 { 5 }
fn default_reconnect_max_delay() -> u32 { 60 }
fn default_reconnect_max_retries() -> u32 { 10 }
fn default_keepalive_interval() -> u32 { 30 }
fn default_keepalive_count() -> u32 { 3 }

fn load_profiles_from_disk() -> Result<Vec<SavedProfile>, String> {
    let config_dir = get_config_path()?;
    let profiles_file = config_dir.join("profiles.json");
    if !profiles_file.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(&profiles_file)
        .map_err(|e| format!("Failed to read profiles file: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse profiles: {}", e))
}

fn write_profiles_to_disk(profiles: &Vec<SavedProfile>) -> Result<(), String> {
    let config_dir = ensure_config_dir()?;
    let profiles_file = config_dir.join("profiles.json");
    let content = serde_json::to_string_pretty(profiles)
        .map_err(|e| format!("Failed to serialize profiles: {}", e))?;
    fs::write(&profiles_file, content).map_err(|e| format!("Failed to write profiles file: {}", e))
}

#[tauri::command]
fn list_profiles() -> Result<Vec<SavedProfile>, String> {
    load_profiles_from_disk()
}

#[tauri::command]
fn save_profile(profile: SavedProfile) -> Result<(), String> {
    let mut profiles = load_profiles_from_disk()?;
    if let Some(pos) = profiles.iter().position(|p| p.id == profile.id) {
        profiles[pos] = profile;
    } else {
        profiles.push(profile);
    }
    write_profiles_to_disk(&profiles)
}

#[tauri::command]
fn delete_profile(id: String) -> Result<(), String> {
    let mut profiles = load_profiles_from_disk()?;
    profiles.retain(|p| p.id != id);
    write_profiles_to_disk(&profiles)
}

// ── SSH Persistent Files ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SshPasswordRecord {
    profile_name: String,
    password: String,
    updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct SshPasswordStore {
    #[serde(default)]
    profiles: Vec<SshPasswordRecord>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct KnownHostRecord {
    profile_name: String,
    host: String,
    port: u16,
    algorithm: String,
    fingerprint: String,
    trusted_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct KnownHostStore {
    #[serde(default)]
    entries: Vec<KnownHostRecord>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SshHostKeyPromptPayload {
    request_id: String,
    profile_name: String,
    host: String,
    port: u16,
    algorithm: String,
    fingerprint: String,
    reason: String,
    known_fingerprint: Option<String>,
}

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn ssh_password_store_path() -> Result<PathBuf, String> {
    Ok(ensure_config_dir()?.join("ssh_profiles.json"))
}

fn ssh_known_hosts_path() -> Result<PathBuf, String> {
    Ok(ensure_config_dir()?.join("ssh_known_hosts.json"))
}

fn load_password_store() -> Result<SshPasswordStore, String> {
    let path = ssh_password_store_path()?;
    if !path.exists() {
        return Ok(SshPasswordStore::default());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read SSH password store: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse SSH password store: {}", e))
}

fn save_password_store(store: &SshPasswordStore) -> Result<(), String> {
    let path = ssh_password_store_path()?;
    let content = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Failed to serialize SSH password store: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("Failed to write SSH password store: {}", e))
}

fn load_known_host_store() -> Result<KnownHostStore, String> {
    let path = ssh_known_hosts_path()?;
    if !path.exists() {
        return Ok(KnownHostStore::default());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read known hosts store: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse known hosts store: {}", e))
}

fn save_known_host_store(store: &KnownHostStore) -> Result<(), String> {
    let path = ssh_known_hosts_path()?;
    let content = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Failed to serialize known hosts store: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("Failed to write known hosts store: {}", e))
}

fn load_password_for_profile(profile_name: &str) -> Result<Option<String>, String> {
    let store = load_password_store()?;
    Ok(store
        .profiles
        .iter()
        .find(|p| p.profile_name == profile_name)
        .map(|p| p.password.clone()))
}

fn save_password_for_profile(profile_name: &str, password: &str) -> Result<(), String> {
    let mut store = load_password_store()?;
    if let Some(existing) = store
        .profiles
        .iter_mut()
        .find(|p| p.profile_name == profile_name)
    {
        existing.password = password.to_string();
        existing.updated_at = now_unix_ms();
    } else {
        store.profiles.push(SshPasswordRecord {
            profile_name: profile_name.to_string(),
            password: password.to_string(),
            updated_at: now_unix_ms(),
        });
    }
    save_password_store(&store)
}

fn load_known_host_by_profile(profile_name: &str) -> Result<Option<KnownHostRecord>, String> {
    let store = load_known_host_store()?;
    Ok(store
        .entries
        .iter()
        .find(|e| e.profile_name == profile_name)
        .cloned())
}

fn save_known_host_entry(entry: KnownHostRecord) -> Result<(), String> {
    let mut store = load_known_host_store()?;
    if let Some(existing) = store
        .entries
        .iter_mut()
        .find(|e| e.profile_name == entry.profile_name)
    {
        *existing = entry;
    } else {
        store.entries.push(entry);
    }
    save_known_host_store(&store)
}

// ── PTY State ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
struct PtyConnectionOptions {
    #[serde(default, rename = "type")]
    connection_type: Option<String>,
    #[serde(default, alias = "profileName")]
    profile_name: Option<String>,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default, alias = "rememberPassword")]
    remember_password: Option<bool>,
    #[serde(default)]
    reconnect: Option<bool>,
    #[serde(default, alias = "reconnectDelaySecs")]
    reconnect_delay_secs: Option<u64>,
    #[serde(default, alias = "reconnectMaxDelaySecs")]
    reconnect_max_delay_secs: Option<u64>,
    #[serde(default, alias = "reconnectMaxRetries")]
    reconnect_max_retries: Option<u16>,
    #[serde(default, alias = "keepaliveIntervalSecs")]
    keepalive_interval_secs: Option<u16>,
    #[serde(default, alias = "keepaliveCountMax")]
    keepalive_count_max: Option<u16>,
    #[serde(default, alias = "privateKeyPath")]
    private_key_path: Option<String>,
    #[serde(default, alias = "privateKeyPassphrase")]
    private_key_passphrase: Option<String>,
}

impl Default for PtyConnectionOptions {
    fn default() -> Self {
        Self {
            connection_type: Some("terminal".to_string()),
            profile_name: None,
            host: None,
            port: None,
            username: None,
            password: None,
            remember_password: None,
            reconnect: None,
            reconnect_delay_secs: None,
            reconnect_max_delay_secs: None,
            reconnect_max_retries: None,
            keepalive_interval_secs: None,
            keepalive_count_max: None,
            private_key_path: None,
            private_key_passphrase: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionKind {
    Terminal,
    Ssh,
}

#[derive(Debug, Clone)]
struct SessionPlan {
    kind: SessionKind,
    profile_name: String,
    host: Option<String>,
    port: u16,
    username: Option<String>,
    password: Option<String>,
    remember_password: bool,
    reconnect: bool,
    reconnect_initial_delay: Duration,
    reconnect_max_delay: Duration,
    reconnect_max_retries: Option<u32>,
    keepalive_interval_secs: u16,
    keepalive_count_max: u16,
    private_key_path: Option<String>,
    private_key_passphrase: Option<String>,
}

struct ActivePty {
    writer: Box<dyn Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send>,
}

struct ActiveSsh {
    input_tx: mpsc::UnboundedSender<Vec<u8>>,
    resize_tx: mpsc::UnboundedSender<(u16, u16)>,
    task: tokio::task::JoinHandle<()>,
}

enum ActiveSession {
    Local(ActivePty),
    Ssh(ActiveSsh),
}

struct PtySession {
    active: Arc<TokioMutex<Option<ActiveSession>>>,
    stop_tx: watch::Sender<bool>,
    supervisor: tokio::task::JoinHandle<()>,
}

struct TokioRuntimeState {
    runtime: tokio::runtime::Runtime,
}

enum SessionExitSignal {
    Terminated,
    Recoverable(String),
    NonRecoverable(String),
}

type PtyMap = Arc<RwLock<HashMap<String, PtySession>>>;
type HostPromptMap = Arc<RwLock<HashMap<String, oneshot::Sender<bool>>>>;

const PTY_OUTPUT_FLUSH_INTERVAL: Duration = Duration::from_millis(8);
const PTY_OUTPUT_MAX_BATCH_BYTES: usize = 256 * 1024;
const HOST_KEY_PROMPT_TIMEOUT: Duration = Duration::from_secs(120);
const HOST_KEY_REJECTED_REASON: &str = "SSH host fingerprint rejected by user";

fn normalize_connection(connection: Option<PtyConnectionOptions>) -> Result<SessionPlan, String> {
    let connection = connection.unwrap_or_default();
    let kind = match connection.connection_type.as_deref() {
        Some("ssh") => SessionKind::Ssh,
        _ => SessionKind::Terminal,
    };

    let reconnect = connection
        .reconnect
        .unwrap_or(matches!(kind, SessionKind::Ssh));
    let reconnect_initial_delay_secs = connection.reconnect_delay_secs.unwrap_or(3).max(1);
    let reconnect_max_delay_secs = connection
        .reconnect_max_delay_secs
        .unwrap_or(60)
        .max(reconnect_initial_delay_secs);
    let reconnect_max_retries = match connection.reconnect_max_retries {
        Some(0) => None,
        Some(value) => Some(value as u32),
        None => Some(8),
    };
    let keepalive_interval_secs = connection.keepalive_interval_secs.unwrap_or(15).max(5);
    let keepalive_count_max = connection.keepalive_count_max.unwrap_or(3).max(1);

    match kind {
        SessionKind::Terminal => Ok(SessionPlan {
            kind,
            profile_name: "terminal".to_string(),
            host: None,
            port: 0,
            username: None,
            password: None,
            remember_password: false,
            reconnect: false,
            reconnect_initial_delay: Duration::from_secs(reconnect_initial_delay_secs),
            reconnect_max_delay: Duration::from_secs(reconnect_max_delay_secs),
            reconnect_max_retries: None,
            keepalive_interval_secs,
            keepalive_count_max,
            private_key_path: None,
            private_key_passphrase: None,
        }),
        SessionKind::Ssh => {
            let host = connection
                .host
                .as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .ok_or_else(|| "SSH host is required".to_string())?;
            let port = connection.port.unwrap_or(22);
            let username = connection
                .username
                .as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .ok_or_else(|| "SSH username is required".to_string())?;

            let profile_name = connection
                .profile_name
                .as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| format!("{}@{}:{}", username, host, port));

            let password = connection.password.filter(|v| !v.is_empty());
            let remember_password = connection.remember_password.unwrap_or(false);
            let private_key_path = connection
                .private_key_path
                .filter(|v| !v.is_empty());
            let private_key_passphrase = connection
                .private_key_passphrase
                .filter(|v| !v.is_empty());

            Ok(SessionPlan {
                kind,
                profile_name,
                host: Some(host),
                port,
                username: Some(username),
                password,
                remember_password,
                reconnect,
                reconnect_initial_delay: Duration::from_secs(reconnect_initial_delay_secs),
                reconnect_max_delay: Duration::from_secs(reconnect_max_delay_secs),
                reconnect_max_retries,
                keepalive_interval_secs,
                keepalive_count_max,
                private_key_path,
                private_key_passphrase,
            })
        }
    }
}

fn resolve_ssh_password(plan: &mut SessionPlan) -> Result<(), String> {
    if !matches!(plan.kind, SessionKind::Ssh) {
        return Ok(());
    }

    // If using key auth, no password needed
    if plan.private_key_path.is_some() {
        return Ok(());
    }

    let password = if let Some(password) = plan.password.clone() {
        password
    } else {
        load_password_for_profile(&plan.profile_name)?.ok_or_else(|| {
            format!(
                "No password provided and no saved password found for profile '{}'",
                plan.profile_name
            )
        })?
    };

    if plan.remember_password {
        save_password_for_profile(&plan.profile_name, &password)?;
    }

    plan.password = Some(password);
    Ok(())
}

fn next_backoff_delay(current: Duration, max: Duration) -> Duration {
    if current >= max {
        return max;
    }

    let doubled = current.saturating_mul(2);
    if doubled > max {
        max
    } else {
        doubled
    }
}

fn emit_pty_output(app: &AppHandle, tab_id: &str, payload: String) {
    let event_name = format!("pty-output-{}", tab_id);
    let _ = app.emit_to(tauri::EventTarget::any(), &event_name, payload);
}

fn emit_pty_exit(app: &AppHandle, tab_id: &str, reason: Option<&str>) {
    let event_name = format!("pty-exit-{}", tab_id);
    let _ = app.emit_to(tauri::EventTarget::any(), &event_name, reason);
}

fn emit_status_line(app: &AppHandle, tab_id: &str, color: &str, message: &str) {
    let payload = format!("\r\n\x1b[{}m[{}]\x1b[0m\r\n", color, message);
    emit_pty_output(app, tab_id, payload);
}

fn emit_batched_output(app: &AppHandle, tab_id: &str, pending_output: &mut String) {
    if pending_output.is_empty() {
        return;
    }
    emit_pty_output(app, tab_id, std::mem::take(pending_output));
}

fn spawn_reader_thread(
    mut reader: Box<dyn Read + Send>,
    app: AppHandle,
    tab_id: String,
    exit_tx: mpsc::UnboundedSender<SessionExitSignal>,
) {
    thread::spawn(move || {
        let mut buf = [0u8; 65536];
        let mut pending_output = String::with_capacity(65536);
        let mut last_flush = Instant::now();

        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]);
                    pending_output.push_str(data.as_ref());

                    if pending_output.len() >= PTY_OUTPUT_MAX_BATCH_BYTES
                        || last_flush.elapsed() >= PTY_OUTPUT_FLUSH_INTERVAL
                    {
                        emit_batched_output(&app, &tab_id, &mut pending_output);
                        last_flush = Instant::now();
                    }
                }
                Err(_) => break,
            }
        }

        emit_batched_output(&app, &tab_id, &mut pending_output);
        let _ = exit_tx.send(SessionExitSignal::Terminated);
    });
}

fn build_terminal_command() -> CommandBuilder {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());

    let mut cmd = CommandBuilder::new(&shell);
    cmd.env("TERM", "xterm-256color");
    cmd.env("HOME", &home);
    cmd.env("LANG", "en_US.UTF-8");
    cmd.cwd(&home);
    cmd
}

#[derive(Clone)]
struct SshClientHandler {
    app: AppHandle,
    tab_id: String,
    profile_name: String,
    host: String,
    port: u16,
    prompts: HostPromptMap,
    user_rejected_host_key: Arc<AtomicBool>,
}

impl client::Handler for SshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let algorithm = server_public_key.algorithm().to_string();
        let fingerprint = server_public_key.fingerprint(HashAlg::Sha256).to_string();

        let known = match load_known_host_by_profile(&self.profile_name) {
            Ok(value) => value,
            Err(err) => {
                emit_status_line(
                    &self.app,
                    &self.tab_id,
                    "31",
                    &format!("Failed to read known host store: {err}"),
                );
                return Ok(false);
            }
        };

        if let Some(record) = &known {
            if record.host == self.host
                && record.port == self.port
                && record.fingerprint == fingerprint
            {
                return Ok(true);
            }
        }

        let reason = if known.is_some() {
            "mismatch".to_string()
        } else {
            "unknown".to_string()
        };
        let known_fingerprint = known.as_ref().map(|r| r.fingerprint.clone());

        let request_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<bool>();
        self.prompts.write().await.insert(request_id.clone(), tx);

        let payload = SshHostKeyPromptPayload {
            request_id: request_id.clone(),
            profile_name: self.profile_name.clone(),
            host: self.host.clone(),
            port: self.port,
            algorithm,
            fingerprint: fingerprint.clone(),
            reason: reason.clone(),
            known_fingerprint,
        };

        let event_name = format!("ssh-hostkey-prompt-{}", self.tab_id);
        let _ = self
            .app
            .emit_to(tauri::EventTarget::any(), &event_name, payload);

        emit_status_line(
            &self.app,
            &self.tab_id,
            "33",
            "Waiting for user confirmation of SSH host fingerprint...",
        );

        let approved = match tokio::time::timeout(HOST_KEY_PROMPT_TIMEOUT, rx).await {
            Ok(Ok(value)) => value,
            Ok(Err(_)) => false,
            Err(_) => {
                let _ = self.prompts.write().await.remove(&request_id);
                false
            }
        };

        if !approved {
            self.user_rejected_host_key.store(true, Ordering::Relaxed);
            emit_status_line(&self.app, &self.tab_id, "31", HOST_KEY_REJECTED_REASON);
            return Err(russh::Error::Disconnect);
        }

        let save_result = save_known_host_entry(KnownHostRecord {
            profile_name: self.profile_name.clone(),
            host: self.host.clone(),
            port: self.port,
            algorithm: server_public_key.algorithm().to_string(),
            fingerprint,
            trusted_at: now_unix_ms(),
        });

        if let Err(err) = save_result {
            emit_status_line(
                &self.app,
                &self.tab_id,
                "31",
                &format!("Failed to save known host: {err}"),
            );
            return Ok(false);
        }

        Ok(true)
    }
}

async fn run_single_ssh_connection(
    app: AppHandle,
    tab_id: String,
    rows: u16,
    cols: u16,
    plan: SessionPlan,
    prompts: HostPromptMap,
    mut stop_rx: watch::Receiver<bool>,
    mut input_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    mut resize_rx: mpsc::UnboundedReceiver<(u16, u16)>,
) -> SessionExitSignal {
    let host = match &plan.host {
        Some(host) => host.clone(),
        None => return SessionExitSignal::Terminated,
    };
    let username = match &plan.username {
        Some(username) => username.clone(),
        None => return SessionExitSignal::Terminated,
    };
    let private_key_path = plan.private_key_path.clone();
    let private_key_passphrase = plan.private_key_passphrase.clone();
    let password = if private_key_path.is_none() {
        match &plan.password {
            Some(password) => password.clone(),
            None => return SessionExitSignal::Terminated,
        }
    } else {
        String::new()
    };

    let mut config = client::Config::default();
    config.keepalive_interval = Some(Duration::from_secs(plan.keepalive_interval_secs as u64));
    config.keepalive_max = plan.keepalive_count_max as usize;

    let handler = SshClientHandler {
        app: app.clone(),
        tab_id: tab_id.clone(),
        profile_name: plan.profile_name.clone(),
        host: host.clone(),
        port: plan.port,
        prompts,
        user_rejected_host_key: Arc::new(AtomicBool::new(false)),
    };

    let host_key_rejected_flag = handler.user_rejected_host_key.clone();

    let mut session = match client::connect(Arc::new(config), (host.as_str(), plan.port), handler).await
    {
        Ok(session) => session,
        Err(err) => {
            if host_key_rejected_flag.load(Ordering::Relaxed) {
                return SessionExitSignal::NonRecoverable(HOST_KEY_REJECTED_REASON.to_string());
            }
            return SessionExitSignal::Recoverable(format!("SSH connect failed: {err}"));
        }
    };

    let auth_result = if let Some(key_path) = private_key_path {
        let key_path = std::path::Path::new(&key_path);
        let key_pair = match russh::keys::load_secret_key(key_path, private_key_passphrase.as_deref()) {
            Ok(kp) => kp,
            Err(err) => {
                return SessionExitSignal::NonRecoverable(format!("Failed to load SSH key: {err}"));
            }
        };
        match session.authenticate_publickey(username, russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key_pair), None)).await {
            Ok(result) => result,
            Err(err) => {
                return SessionExitSignal::NonRecoverable(format!("SSH key authentication failed: {err}"));
            }
        }
    } else {
        match session.authenticate_password(username, password).await {
            Ok(result) => result,
            Err(err) => {
                return SessionExitSignal::NonRecoverable(format!("SSH authentication failed: {err}"));
            }
        }
    };

    if !auth_result.success() {
        emit_status_line(&app, &tab_id, "31", "SSH authentication failed");
        let _ = session
            .disconnect(Disconnect::ByApplication, "Authentication failed", "en")
            .await;
        return SessionExitSignal::NonRecoverable("SSH authentication failed".to_string());
    }

    let channel = match session.channel_open_session().await {
        Ok(channel) => channel,
        Err(err) => {
            return SessionExitSignal::Recoverable(format!("Failed to open SSH channel: {err}"));
        }
    };

    if let Err(err) = channel
        .request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
        .await
    {
        return SessionExitSignal::Recoverable(format!("Failed to request SSH PTY: {err}"));
    }

    if let Err(err) = channel.request_shell(false).await {
        return SessionExitSignal::Recoverable(format!("Failed to request SSH shell: {err}"));
    }

    let (mut reader, writer) = channel.split();
    let mut writer_stream = writer.make_writer();

    loop {
        tokio::select! {
            _ = stop_rx.changed() => {
                if *stop_rx.borrow() {
                    let _ = writer.close().await;
                    let _ = session.disconnect(Disconnect::ByApplication, "Session closed", "en").await;
                    return SessionExitSignal::Terminated;
                }
            }
            incoming = input_rx.recv() => {
                if let Some(data) = incoming {
                    if let Err(err) = writer_stream.write_all(&data).await {
                        return SessionExitSignal::Recoverable(format!("SSH write failed: {err}"));
                    }
                }
            }
            resize = resize_rx.recv() => {
                if let Some((next_rows, next_cols)) = resize {
                    if let Err(err) = writer.window_change(next_cols as u32, next_rows as u32, 0, 0).await {
                        return SessionExitSignal::Recoverable(format!("SSH resize failed: {err}"));
                    }
                }
            }
            event = reader.wait() => {
                match event {
                    Some(ChannelMsg::Data { data }) => {
                        let text = String::from_utf8_lossy(data.as_ref()).to_string();
                        emit_pty_output(&app, &tab_id, text);
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        let text = String::from_utf8_lossy(data.as_ref()).to_string();
                        emit_pty_output(&app, &tab_id, text);
                    }
                    Some(ChannelMsg::ExitStatus { .. }) => {
                        let _ = session.disconnect(Disconnect::ByApplication, "Shell exited", "en").await;
                        return SessionExitSignal::Terminated;
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => {
                        return SessionExitSignal::Recoverable("SSH channel closed".to_string());
                    }
                    _ => {}
                }
            }
        }
    }
}

fn spawn_process(
    app: &AppHandle,
    tab_id: &str,
    rows: u16,
    cols: u16,
    plan: &SessionPlan,
    stop_rx: watch::Receiver<bool>,
    exit_tx: mpsc::UnboundedSender<SessionExitSignal>,
    prompt_state: HostPromptMap,
    runtime_handle: &tokio::runtime::Handle,
) -> Result<(u32, ActiveSession), String> {
    match plan.kind {
        SessionKind::Terminal => {
            let pty_system = native_pty_system();
            let pair = pty_system
                .openpty(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| format!("Failed to open PTY: {}", e))?;

            let cmd = build_terminal_command();
            let child = pair
                .slave
                .spawn_command(cmd)
                .map_err(|e| format!("Failed to spawn process: {}", e))?;

            let pid = child.process_id().unwrap_or(0);
            drop(pair.slave);

            let reader = pair
                .master
                .try_clone_reader()
                .map_err(|e| format!("Failed to clone reader: {}", e))?;

            spawn_reader_thread(reader, app.clone(), tab_id.to_string(), exit_tx);

            let writer = pair
                .master
                .take_writer()
                .map_err(|e| format!("Failed to take writer: {}", e))?;

            let active = ActiveSession::Local(ActivePty {
                writer,
                master: pair.master,
                child,
            });

            Ok((pid, active))
        }
        SessionKind::Ssh => {
            let (input_tx, input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
            let (resize_tx, resize_rx) = mpsc::unbounded_channel::<(u16, u16)>();

            let app_clone = app.clone();
            let tab_id_clone = tab_id.to_string();
            let plan_clone = plan.clone();
            let prompt_state_clone = prompt_state;
            let exit_tx_clone = exit_tx.clone();
            let stop_rx_clone = stop_rx.clone();

            let task = runtime_handle.spawn(async move {
                let signal = run_single_ssh_connection(
                    app_clone,
                    tab_id_clone,
                    rows,
                    cols,
                    plan_clone,
                    prompt_state_clone,
                    stop_rx_clone,
                    input_rx,
                    resize_rx,
                )
                .await;
                let _ = exit_tx_clone.send(signal);
            });

            let active = ActiveSession::Ssh(ActiveSsh {
                input_tx,
                resize_tx,
                task,
            });

            Ok((0, active))
        }
    }
}

// ── PTY Commands ──────────────────────────────────────────────────────────────

#[tauri::command]
fn create_pty(
    app: AppHandle,
    tab_id: String,
    rows: u16,
    cols: u16,
    connection: Option<PtyConnectionOptions>,
    state: tauri::State<'_, PtyMap>,
    prompt_state: tauri::State<'_, HostPromptMap>,
    runtime_state: tauri::State<'_, TokioRuntimeState>,
) -> Result<u32, String> {
    let mut plan = normalize_connection(connection)?;
    resolve_ssh_password(&mut plan)?;

    {
        let exists = runtime_state
            .runtime
            .block_on(async { state.read().await.contains_key(&tab_id) });
        if exists {
            return Err(format!("PTY session {} already exists", tab_id));
        }
    }

    let (exit_tx, mut exit_rx) = mpsc::unbounded_channel::<SessionExitSignal>();
    let active = Arc::new(TokioMutex::new(None));
    let (stop_tx, mut stop_rx) = watch::channel(false);

    let runtime_handle = runtime_state.runtime.handle().clone();

    let (pid, initial_active) = spawn_process(
        &app,
        &tab_id,
        rows,
        cols,
        &plan,
        stop_rx.clone(),
        exit_tx.clone(),
        prompt_state.inner().clone(),
        &runtime_handle,
    )?;

    {
        let mut guard = runtime_state.runtime.block_on(active.lock());
        *guard = Some(initial_active);
    }

    let app_clone = app.clone();
    let tab_id_clone = tab_id.clone();
    let plan_clone = plan.clone();
    let active_clone = active.clone();
    let exit_tx_clone = exit_tx.clone();
    let prompt_state_clone = prompt_state.inner().clone();
    let reconnect_runtime_handle = runtime_handle.clone();

    let supervisor = runtime_state.runtime.spawn(async move {
        let mut attempt: u32 = 0;
        let mut delay = plan_clone.reconnect_initial_delay;

        while let Some(signal) = exit_rx.recv().await {
            if *stop_rx.borrow() {
                break;
            }

            let host_key_rejected = matches!(
                &signal,
                SessionExitSignal::NonRecoverable(reason) if reason == HOST_KEY_REJECTED_REASON
            );

            {
                let mut guard = active_clone.lock().await;
                *guard = None;
            }

            let should_reconnect = (matches!(signal, SessionExitSignal::Recoverable(_))
                || host_key_rejected)
                && matches!(plan_clone.kind, SessionKind::Ssh)
                && plan_clone.reconnect;

            if !should_reconnect {
                let exit_reason = if let SessionExitSignal::Recoverable(reason)
                | SessionExitSignal::NonRecoverable(reason) = &signal
                {
                    emit_status_line(&app_clone, &tab_id_clone, "31", reason);
                    Some(reason.clone())
                } else {
                    None
                };
                emit_pty_exit(&app_clone, &tab_id_clone, exit_reason.as_deref());
                break;
            }

            if let Some(max_retries) = plan_clone.reconnect_max_retries {
                if attempt >= max_retries {
                    emit_status_line(
                        &app_clone,
                        &tab_id_clone,
                        "31",
                        "SSH reconnect exhausted retry budget",
                    );
                    emit_pty_exit(&app_clone, &tab_id_clone, Some("SSH reconnect exhausted retry budget"));
                    break;
                }
            }

            attempt += 1;

            emit_status_line(
                &app_clone,
                &tab_id_clone,
                "33",
                &format!(
                    "SSH disconnected. Reconnect attempt {attempt} in {}s...",
                    delay.as_secs()
                ),
            );

            loop {
                tokio::select! {
                    _ = stop_rx.changed() => {
                        return;
                    }
                    _ = tokio::time::sleep(delay) => {}
                }

                if *stop_rx.borrow() {
                    return;
                }

                match spawn_process(
                    &app_clone,
                    &tab_id_clone,
                    rows,
                    cols,
                    &plan_clone,
                    stop_rx.clone(),
                    exit_tx_clone.clone(),
                    prompt_state_clone.clone(),
                    &reconnect_runtime_handle,
                ) {
                    Ok((_, new_active)) => {
                        let mut guard = active_clone.lock().await;
                        *guard = Some(new_active);
                        emit_status_line(&app_clone, &tab_id_clone, "32", "SSH reconnected");
                        if !host_key_rejected {
                            attempt = 0;
                            delay = plan_clone.reconnect_initial_delay;
                        }
                        break;
                    }
                    Err(err) => {
                        delay = next_backoff_delay(delay, plan_clone.reconnect_max_delay);
                        emit_status_line(
                            &app_clone,
                            &tab_id_clone,
                            "31",
                            &format!("Reconnect failed: {}", err),
                        );
                    }
                }
            }
        }
    });

    let session = PtySession {
        active,
        stop_tx,
        supervisor,
    };

    runtime_state
        .runtime
        .block_on(async { state.write().await.insert(tab_id, session) });

    Ok(pid)
}

#[tauri::command]
fn write_pty(tab_id: String, data: String, state: tauri::State<'_, PtyMap>) -> Result<(), String> {
    let map = state.blocking_read();
    let session = map
        .get(&tab_id)
        .ok_or_else(|| format!("PTY session {} not found", tab_id))?;

    let mut active_guard = session.active.blocking_lock();
    let active = active_guard
        .as_mut()
        .ok_or_else(|| format!("PTY session {} is reconnecting", tab_id))?;

    match active {
        ActiveSession::Local(local) => local
            .writer
            .write_all(data.as_bytes())
            .map_err(|e| format!("Failed to write to PTY: {}", e)),
        ActiveSession::Ssh(ssh) => ssh
            .input_tx
            .send(data.into_bytes())
            .map_err(|_| format!("PTY session {} is not writable", tab_id)),
    }
}

#[tauri::command]
fn resize_pty(
    tab_id: String,
    rows: u16,
    cols: u16,
    state: tauri::State<'_, PtyMap>,
) -> Result<(), String> {
    let map = state.blocking_read();
    let session = map
        .get(&tab_id)
        .ok_or_else(|| format!("PTY session {} not found", tab_id))?;

    let mut active_guard = session.active.blocking_lock();
    let active = active_guard
        .as_mut()
        .ok_or_else(|| format!("PTY session {} is reconnecting", tab_id))?;

    match active {
        ActiveSession::Local(local) => local
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to resize PTY: {}", e)),
        ActiveSession::Ssh(ssh) => ssh
            .resize_tx
            .send((rows, cols))
            .map_err(|_| format!("PTY session {} is not resizable", tab_id)),
    }
}

#[tauri::command]
fn kill_pty(tab_id: String, state: tauri::State<'_, PtyMap>) -> Result<(), String> {
    let session = state.blocking_write().remove(&tab_id);

    if let Some(session) = session {
        let _ = session.stop_tx.send(true);

        if let Some(active) = session.active.blocking_lock().take() {
            match active {
                ActiveSession::Local(mut local) => {
                    let _ = local.child.kill();
                }
                ActiveSession::Ssh(ssh) => {
                    ssh.task.abort();
                }
            }
        }

        session.supervisor.abort();
    }

    Ok(())
}

#[tauri::command]
fn respond_ssh_host_key_prompt(
    request_id: String,
    trust: bool,
    prompt_state: tauri::State<'_, HostPromptMap>,
) -> Result<(), String> {
    let sender = prompt_state
        .blocking_write()
        .remove(&request_id)
        .ok_or_else(|| "Host key prompt expired".to_string())?;

    sender
        .send(trust)
        .map_err(|_| "Host key prompt receiver is gone".to_string())
}

// ── Font Discovery ────────────────────────────────────────────────────────────

#[tauri::command]
fn list_fonts() -> Vec<String> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/System/Library/Fonts"));
        dirs.push(PathBuf::from("/Library/Fonts"));
        if let Ok(home) = std::env::var("HOME") {
            dirs.push(PathBuf::from(&home).join("Library/Fonts"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(windir) = std::env::var("WINDIR") {
            dirs.push(PathBuf::from(&windir).join("Fonts"));
        } else {
            dirs.push(PathBuf::from("C:/Windows/Fonts"));
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            dirs.push(PathBuf::from(&appdata).join("Microsoft/Windows/Fonts"));
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        dirs.push(PathBuf::from("/usr/share/fonts"));
        dirs.push(PathBuf::from("/usr/local/share/fonts"));
        if let Ok(home) = std::env::var("HOME") {
            dirs.push(PathBuf::from(&home).join(".fonts"));
            dirs.push(PathBuf::from(&home).join(".local/share/fonts"));
        }
    }

    let mut font_names: HashSet<String> = HashSet::new();

    for dir in &dirs {
        collect_fonts_from_dir(dir, &mut font_names);
    }

    let mut result: Vec<String> = font_names.into_iter().collect();
    result.sort();
    result
}

fn collect_fonts_from_dir(dir: &PathBuf, names: &mut HashSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_fonts_from_dir(&path, names);
        } else if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            if matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
                if let Some(name) = extract_font_name(&path) {
                    names.insert(name);
                }
            }
        }
    }
}

fn extract_font_name(path: &PathBuf) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let suffixes = [
        "-Bold",
        "-Italic",
        "-BoldItalic",
        "-Regular",
        "-Light",
        "-Medium",
        "-Thin",
        "-Black",
        "-Heavy",
        "-SemiBold",
        "-ExtraBold",
        "-ExtraLight",
        "-Condensed",
        "-Oblique",
        "-Mono",
        "-NF",
        "-NerdFont",
        "Bold",
        "Italic",
        "Regular",
        "Light",
        "Medium",
    ];
    let mut name = stem.clone();
    for suffix in &suffixes {
        if let Some(stripped) = name.strip_suffix(suffix) {
            name = stripped
                .trim_end_matches('-')
                .trim_end_matches(' ')
                .to_string();
            break;
        }
    }

    let display = name.replace('_', " ");
    if display.is_empty() {
        None
    } else {
        Some(display)
    }
}

// ── App Setup ─────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let pty_map: PtyMap = Arc::new(RwLock::new(HashMap::new()));
    let host_prompt_map: HostPromptMap = Arc::new(RwLock::new(HashMap::new()));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(pty_map)
        .manage(host_prompt_map)
        .manage(TokioRuntimeState { runtime })
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            load_session,
            save_session,
            clear_session,
            create_pty,
            write_pty,
            resize_pty,
            kill_pty,
            respond_ssh_host_key_prompt,
            list_fonts,
            list_profiles,
            save_profile,
            delete_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
