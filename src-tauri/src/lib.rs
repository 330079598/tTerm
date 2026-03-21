use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AppConfig {
    theme: String,
    language: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
            language: "en".to_string(),
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
    fs::write(&config_file, content)
        .map_err(|e| format!("Failed to write config file: {}", e))
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
    fs::write(&session_file, content)
        .map_err(|e| format!("Failed to write session file: {}", e))
}

// ── PTY State ─────────────────────────────────────────────────────────────────

struct PtySession {
    writer: Box<dyn Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    _child: Box<dyn portable_pty::Child + Send>,
}

type PtyMap = Arc<Mutex<HashMap<String, PtySession>>>;

// ── PTY Commands ──────────────────────────────────────────────────────────────

#[tauri::command]
fn create_pty(
    app: AppHandle,
    tab_id: String,
    rows: u16,
    cols: u16,
    state: tauri::State<'_, PtyMap>,
) -> Result<u32, String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());

    let mut cmd = CommandBuilder::new(&shell);
    cmd.env("TERM", "xterm-256color");
    cmd.env("HOME", &home);
    cmd.env("LANG", "en_US.UTF-8");
    cmd.cwd(&home);

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {}", e))?;

    let pid = child.process_id().unwrap_or(0);

    // Keep child alive by storing it; drop slave after spawning
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone reader: {}", e))?;

    let app_clone = app.clone();
    let tab_id_clone = tab_id.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app_clone.emit_to(
                        tauri::EventTarget::any(),
                        &format!("pty-output-{}", tab_id_clone),
                        data,
                    );
                }
                Err(_) => break,
            }
        }
        let _ = app_clone.emit_to(
            tauri::EventTarget::any(),
            &format!("pty-exit-{}", tab_id_clone),
            (),
        );
    });

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to take writer: {}", e))?;

    let session = PtySession {
        writer,
        master: pair.master,
        _child: child,
    };

    state.lock().unwrap().insert(tab_id, session);

    Ok(pid)
}

#[tauri::command]
fn write_pty(
    tab_id: String,
    data: String,
    state: tauri::State<'_, PtyMap>,
) -> Result<(), String> {
    let mut map = state.lock().unwrap();
    let session = map
        .get_mut(&tab_id)
        .ok_or_else(|| format!("PTY session {} not found", tab_id))?;
    session
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
    let map = state.lock().unwrap();
    let session = map
        .get(&tab_id)
        .ok_or_else(|| format!("PTY session {} not found", tab_id))?;
    session
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
    state.lock().unwrap().remove(&tab_id);
    Ok(())
}

// ── App Setup ─────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let pty_map: PtyMap = Arc::new(Mutex::new(HashMap::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .manage(pty_map)
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            load_session,
            save_session,
            create_pty,
            write_pty,
            resize_pty,
            kill_pty,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
