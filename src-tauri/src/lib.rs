use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::{watch, Mutex as TokioMutex, RwLock};

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

// ── PTY State ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
struct PtyConnectionOptions {
    #[serde(default, rename = "type")]
    connection_type: Option<String>,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    username: Option<String>,
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
}

impl Default for PtyConnectionOptions {
    fn default() -> Self {
        Self {
            connection_type: Some("terminal".to_string()),
            host: None,
            port: None,
            username: None,
            reconnect: None,
            reconnect_delay_secs: None,
            reconnect_max_delay_secs: None,
            reconnect_max_retries: None,
            keepalive_interval_secs: None,
            keepalive_count_max: None,
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
    host: Option<String>,
    port: u16,
    username: Option<String>,
    reconnect: bool,
    reconnect_initial_delay: Duration,
    reconnect_max_delay: Duration,
    reconnect_max_retries: Option<u32>,
    keepalive_interval_secs: u16,
    keepalive_count_max: u16,
}

struct ActivePty {
    writer: Box<dyn Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send>,
}

struct PtySession {
    active: Arc<TokioMutex<Option<ActivePty>>>,
    stop_tx: watch::Sender<bool>,
    supervisor: tokio::task::JoinHandle<()>,
}

struct TokioRuntimeState {
    runtime: tokio::runtime::Runtime,
}

type PtyMap = Arc<RwLock<HashMap<String, PtySession>>>;

const PTY_OUTPUT_FLUSH_INTERVAL: Duration = Duration::from_millis(8);
const PTY_OUTPUT_MAX_BATCH_BYTES: usize = 256 * 1024;

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
            host: None,
            port: 0,
            username: None,
            reconnect: false,
            reconnect_initial_delay: Duration::from_secs(reconnect_initial_delay_secs),
            reconnect_max_delay: Duration::from_secs(reconnect_max_delay_secs),
            reconnect_max_retries: None,
            keepalive_interval_secs,
            keepalive_count_max,
        }),
        SessionKind::Ssh => {
            let host = connection
                .host
                .as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .ok_or_else(|| "SSH host is required".to_string())?;
            let username = connection
                .username
                .as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());

            Ok(SessionPlan {
                kind,
                host: Some(host),
                port: connection.port.unwrap_or(22),
                username,
                reconnect,
                reconnect_initial_delay: Duration::from_secs(reconnect_initial_delay_secs),
                reconnect_max_delay: Duration::from_secs(reconnect_max_delay_secs),
                reconnect_max_retries,
                keepalive_interval_secs,
                keepalive_count_max,
            })
        }
    }
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

fn emit_pty_exit(app: &AppHandle, tab_id: &str) {
    let event_name = format!("pty-exit-{}", tab_id);
    let _ = app.emit_to(tauri::EventTarget::any(), &event_name, ());
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
    exit_tx: tokio::sync::mpsc::UnboundedSender<()>,
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
        let _ = exit_tx.send(());
    });
}

fn build_command(plan: &SessionPlan) -> Result<CommandBuilder, String> {
    match plan.kind {
        SessionKind::Terminal => {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
            let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());

            let mut cmd = CommandBuilder::new(&shell);
            cmd.env("TERM", "xterm-256color");
            cmd.env("HOME", &home);
            cmd.env("LANG", "en_US.UTF-8");
            cmd.cwd(&home);
            Ok(cmd)
        }
        SessionKind::Ssh => {
            let host = plan
                .host
                .as_ref()
                .ok_or_else(|| "SSH host is missing".to_string())?;

            let mut cmd = CommandBuilder::new("ssh");
            cmd.arg("-tt");
            cmd.arg("-p");
            cmd.arg(plan.port.to_string());
            cmd.arg("-o");
            cmd.arg(format!(
                "ServerAliveInterval={}",
                plan.keepalive_interval_secs
            ));
            cmd.arg("-o");
            cmd.arg(format!("ServerAliveCountMax={}", plan.keepalive_count_max));
            cmd.arg("-o");
            cmd.arg("TCPKeepAlive=yes");

            let target = match &plan.username {
                Some(username) => format!("{}@{}", username, host),
                None => host.to_string(),
            };
            cmd.arg(target);
            cmd.env("TERM", "xterm-256color");
            Ok(cmd)
        }
    }
}

fn spawn_process(
    app: &AppHandle,
    tab_id: &str,
    rows: u16,
    cols: u16,
    plan: &SessionPlan,
    exit_tx: tokio::sync::mpsc::UnboundedSender<()>,
) -> Result<(u32, ActivePty), String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    let cmd = build_command(plan)?;
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

    let active = ActivePty {
        writer,
        master: pair.master,
        child,
    };

    Ok((pid, active))
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
    runtime_state: tauri::State<'_, TokioRuntimeState>,
) -> Result<u32, String> {
    let plan = normalize_connection(connection)?;

    {
        let map = runtime_state
            .runtime
            .block_on(async { state.read().await.contains_key(&tab_id) });
        if map {
            return Err(format!("PTY session {} already exists", tab_id));
        }
    }

    let (exit_tx, mut exit_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let active = Arc::new(TokioMutex::new(None));

    let (pid, initial_active) = spawn_process(&app, &tab_id, rows, cols, &plan, exit_tx.clone())?;
    {
        let mut guard = runtime_state.runtime.block_on(active.lock());
        *guard = Some(initial_active);
    }

    let (stop_tx, mut stop_rx) = watch::channel(false);

    let app_clone = app.clone();
    let tab_id_clone = tab_id.clone();
    let plan_clone = plan.clone();
    let active_clone = active.clone();
    let exit_tx_clone = exit_tx.clone();

    let supervisor = runtime_state.runtime.spawn(async move {
        let mut attempt: u32 = 0;
        let mut delay = plan_clone.reconnect_initial_delay;

        while exit_rx.recv().await.is_some() {
            if *stop_rx.borrow() {
                break;
            }

            {
                let mut guard = active_clone.lock().await;
                *guard = None;
            }

            let should_reconnect =
                matches!(plan_clone.kind, SessionKind::Ssh) && plan_clone.reconnect;
            if !should_reconnect {
                emit_pty_exit(&app_clone, &tab_id_clone);
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
                    emit_pty_exit(&app_clone, &tab_id_clone);
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
                    exit_tx_clone.clone(),
                ) {
                    Ok((_, new_active)) => {
                        let mut guard = active_clone.lock().await;
                        *guard = Some(new_active);
                        emit_status_line(&app_clone, &tab_id_clone, "32", "SSH reconnected");
                        attempt = 0;
                        delay = plan_clone.reconnect_initial_delay;
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

    active
        .writer
        .write_all(data.as_bytes())
        .map_err(|e| format!("Failed to write to PTY: {}", e))
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

    active
        .master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to resize PTY: {}", e))
}

#[tauri::command]
fn kill_pty(tab_id: String, state: tauri::State<'_, PtyMap>) -> Result<(), String> {
    let session = state.blocking_write().remove(&tab_id);

    if let Some(session) = session {
        let _ = session.stop_tx.send(true);

        if let Some(mut active) = session.active.blocking_lock().take() {
            let _ = active.child.kill();
        }

        session.supervisor.abort();
    }

    Ok(())
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
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .manage(pty_map)
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
            list_fonts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
