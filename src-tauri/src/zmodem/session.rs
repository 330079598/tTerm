//! Drives one tab's ZMODEM receive flow: detection, the protocol engine,
//! destination-file I/O, and progress events. Deliberately write-mechanism
//! agnostic — it returns bytes to write rather than writing them itself —
//! since the local-PTY reader thread (sync, `Write::write_all`) and the
//! async SSH channel task (later milestone, `AsyncWriteExt::write_all`)
//! need to send them back through completely different mechanisms.

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use super::detect;
use super::events::{
    self, EmitFn, TransferCompletePayload, TransferProgressPayload, TransferStartPayload,
};
use super::protocol::consts::ZDLE;
use super::protocol::{ZmodemAction, ZmodemDirection, ZmodemEngine};
use crate::core::{ZmodemMap, ZmodemTabHandle};

/// How often (at most) a data chunk triggers a progress event, so a fast
/// local transfer doesn't flood the frontend with one IPC message per
/// ~1KB subpacket.
const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(120);

/// The classic ZMODEM abort signal: enough consecutive `CAN` bytes that no
/// legitimate escape sequence could produce them, recognized by `engine.rs`'s
/// `detect_abort` and by any real `rz`/`sz` peer.
pub fn abort_sequence() -> Vec<u8> {
    vec![ZDLE; 5]
}

pub struct ZmodemStepOutcome {
    /// Bytes that are ordinary terminal output and should reach the webview
    /// exactly as if no ZMODEM handling existed.
    pub passthrough: Vec<u8>,
    /// Bytes to write back to the peer (ZMODEM replies), in order.
    pub outgoing: Vec<u8>,
    /// Set once a `rz`-style (send-direction) trigger is detected, carrying
    /// the trigger bytes onward. This driver only ever receives, so on this
    /// it does nothing further for the tab — the caller must switch the tab
    /// into piped-to-send-session mode (see `ZmodemArmedSend`), starting
    /// with these bytes, instead of calling `process()` again.
    pub send_requested: Option<Vec<u8>>,
}

impl ZmodemStepOutcome {
    fn empty() -> Self {
        Self {
            passthrough: Vec::new(),
            outgoing: Vec::new(),
            send_requested: None,
        }
    }
}

struct FileState {
    handle: File,
    written: u64,
    started_at: Instant,
    last_progress_emit_at: Instant,
}

struct InFlightTransfer {
    engine: ZmodemEngine,
    transfer_id: String,
    cancel_requested: Arc<AtomicBool>,
    file: Option<FileState>,
}

/// Owns one tab's ZMODEM receive state across many `process()` calls. Not
/// `Clone`/`Sync` on purpose: exactly one thread/task ever drives a given
/// channel's bytes, so this never needs to be shared — only the thin
/// `ZmodemTabHandle` published into `ZmodemMap` is.
///
/// Progress/start/complete notifications go through an injected callback
/// rather than a stored `AppHandle` directly, mirroring `terminal::pty::
/// run_pty_reader`'s injected-closures pattern: it decouples this driver
/// from a concrete Tauri runtime, which is what lets integration tests
/// drive it against a real local PTY + real `sz` process without needing a
/// real windowed `AppHandle` (Tauri's `MockRuntime` test handle is a
/// different, incompatible type from the `AppHandle` production code uses).
pub struct ZmodemReceiveDriver {
    emit: EmitFn,
    tab_id: String,
    session_nonce: u32,
    zmodem_map: ZmodemMap,
    download_dir: PathBuf,
    auto_detect_enabled: bool,
    /// Flipped by the `zmodem_arm_manual_detect` command (the "Send/Receive
    /// Files via ZMODEM" keymap actions): forces detection on for this
    /// tab's next trigger even when `auto_detect_enabled` is false. Sticky
    /// once set (cleared only when a trigger actually fires) rather than
    /// time-limited — simpler, and this is an explicit, rare, user-initiated
    /// action, not passive background scanning.
    manual_override: Arc<AtomicBool>,
    detect_state: detect::DetectState,
    transfer: Option<InFlightTransfer>,
}

impl ZmodemReceiveDriver {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app: AppHandle,
        tab_id: String,
        session_nonce: u32,
        zmodem_map: ZmodemMap,
        auto_detect_enabled: bool,
        download_dir: PathBuf,
        manual_override: Arc<AtomicBool>,
    ) -> Self {
        let emit_tab_id = tab_id.clone();
        Self::with_emit(
            Box::new(move |kind, payload| {
                let event = format!("zmodem-{kind}-{emit_tab_id}");
                let _ = app.emit_to(tauri::EventTarget::any(), &event, payload);
            }),
            tab_id,
            session_nonce,
            zmodem_map,
            auto_detect_enabled,
            download_dir,
            manual_override,
        )
    }

    /// Test-only entry point: same driving logic, but notifications go to an
    /// injected closure instead of a real `AppHandle`/webview.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub fn with_test_emit(
        emit: impl Fn(&str, serde_json::Value) + Send + 'static,
        tab_id: String,
        session_nonce: u32,
        zmodem_map: ZmodemMap,
        auto_detect_enabled: bool,
        download_dir: PathBuf,
    ) -> Self {
        Self::with_emit(
            Box::new(emit),
            tab_id,
            session_nonce,
            zmodem_map,
            auto_detect_enabled,
            download_dir,
            Arc::new(AtomicBool::new(false)),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn with_emit(
        emit: EmitFn,
        tab_id: String,
        session_nonce: u32,
        zmodem_map: ZmodemMap,
        auto_detect_enabled: bool,
        download_dir: PathBuf,
        manual_override: Arc<AtomicBool>,
    ) -> Self {
        Self {
            emit,
            tab_id,
            session_nonce,
            zmodem_map,
            download_dir,
            auto_detect_enabled,
            manual_override,
            detect_state: detect::DetectState::default(),
            transfer: None,
        }
    }

    /// Test-only: simulates the "Send/Receive Files via ZMODEM" keymap
    /// action forcing detection on regardless of `auto_detect_enabled`.
    #[cfg(test)]
    pub fn force_manual_override(&self) {
        self.manual_override.store(true, Ordering::Relaxed);
    }

    fn emit_event<T: serde::Serialize>(&self, kind: &str, payload: T) {
        events::emit(&self.emit, kind, payload);
    }

    /// Processes one chunk of incoming bytes.
    pub fn process(&mut self, chunk: &[u8]) -> ZmodemStepOutcome {
        if let Some(transfer) = &self.transfer {
            if transfer.cancel_requested.load(Ordering::Relaxed) {
                self.on_local_cancel();
                return ZmodemStepOutcome {
                    passthrough: chunk.to_vec(),
                    outgoing: Vec::new(),
                    send_requested: None,
                };
            }
        }

        if self.transfer.is_none() {
            if !self.auto_detect_enabled && !self.manual_override.load(Ordering::Relaxed) {
                return ZmodemStepOutcome {
                    passthrough: chunk.to_vec(),
                    outgoing: Vec::new(),
                    send_requested: None,
                };
            }
            let scan = detect::scan(&mut self.detect_state, chunk);
            let mut outcome = ZmodemStepOutcome {
                passthrough: scan.passthrough,
                outgoing: Vec::new(),
                send_requested: None,
            };
            let Some((direction, remainder)) = scan.triggered else {
                return outcome;
            };
            // Consumed: a manual arm covers one trigger, not indefinite
            // scanning beyond it.
            self.manual_override.store(false, Ordering::Relaxed);
            if direction == ZmodemDirection::Send {
                outcome.send_requested = Some(remainder);
                return outcome;
            }
            self.start_transfer(direction);
            self.feed(&remainder, &mut outcome);
            return outcome;
        }

        let mut outcome = ZmodemStepOutcome::empty();
        self.feed(chunk, &mut outcome);
        outcome
    }

    fn feed(&mut self, chunk: &[u8], outcome: &mut ZmodemStepOutcome) {
        let Some(transfer) = self.transfer.as_mut() else {
            outcome.passthrough.extend_from_slice(chunk);
            return;
        };
        let result = transfer.engine.feed(chunk);
        outcome.outgoing.extend_from_slice(&result.outgoing);
        outcome.passthrough.extend_from_slice(&result.passthrough);
        for action in result.actions {
            self.apply_action(action, outcome);
        }
    }

    fn apply_action(&mut self, action: ZmodemAction, outcome: &mut ZmodemStepOutcome) {
        match action {
            ZmodemAction::IncomingFile { name, size } => self.on_incoming_file(name, size, outcome),
            ZmodemAction::DataChunk(bytes) => self.on_data_chunk(&bytes),
            ZmodemAction::FileComplete => self.on_file_complete(),
            ZmodemAction::SessionEnded => self.on_session_ended(),
            ZmodemAction::PeerCancelled => self.on_peer_cancelled(),
            // Send-direction-only action; this driver only ever receives.
            ZmodemAction::ResendFrom(_) => {}
        }
    }

    fn start_transfer(&mut self, direction: ZmodemDirection) {
        let transfer_id = events::new_transfer_id();
        let cancel_requested = Arc::new(AtomicBool::new(false));
        self.zmodem_map.write().unwrap().insert(
            self.tab_id.clone(),
            ZmodemTabHandle {
                session_nonce: self.session_nonce,
                transfer_id: transfer_id.clone(),
                cancel_requested: cancel_requested.clone(),
            },
        );
        self.transfer = Some(InFlightTransfer {
            engine: ZmodemEngine::new(direction),
            transfer_id,
            cancel_requested,
            file: None,
        });
    }

    fn on_incoming_file(&mut self, name: String, size: u64, outcome: &mut ZmodemStepOutcome) {
        let dest_path = unique_destination_path(&self.download_dir, &name);
        let opened = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&dest_path);
        let Some(transfer) = self.transfer.as_mut() else {
            return;
        };
        match opened {
            Ok(handle) => {
                transfer.file = Some(FileState {
                    handle,
                    written: 0,
                    started_at: Instant::now(),
                    last_progress_emit_at: Instant::now(),
                });
                outcome
                    .outgoing
                    .extend_from_slice(&transfer.engine.accept_file(0));
                let transfer_id = transfer.transfer_id.clone();
                self.emit_event(
                    "transfer-start",
                    TransferStartPayload {
                        transfer_id,
                        direction: "download",
                        file_name: name,
                        file_size: size,
                        local_path: dest_path.to_string_lossy().into_owned(),
                    },
                );
            }
            Err(err) => {
                eprintln!(
                    "ZMODEM: failed to create destination file {}: {err}",
                    dest_path.display()
                );
                outcome
                    .outgoing
                    .extend_from_slice(&transfer.engine.skip_file());
            }
        }
    }

    fn on_data_chunk(&mut self, bytes: &[u8]) {
        let Some(transfer) = self.transfer.as_mut() else {
            return;
        };
        let Some(file) = transfer.file.as_mut() else {
            return;
        };
        if let Err(err) = file.handle.write_all(bytes) {
            eprintln!("ZMODEM: write to destination file failed: {err}");
            return;
        }
        file.written += bytes.len() as u64;
        let now = Instant::now();
        if now.duration_since(file.last_progress_emit_at) < PROGRESS_EMIT_INTERVAL {
            return;
        }
        file.last_progress_emit_at = now;
        let elapsed = now.duration_since(file.started_at).as_secs_f64().max(0.001);
        let speed = (file.written as f64 / elapsed) as u64;
        let transferred = file.written;
        let transfer_id = transfer.transfer_id.clone();
        self.emit_event(
            "transfer-progress",
            TransferProgressPayload {
                transfer_id,
                transferred,
                speed,
            },
        );
    }

    fn on_file_complete(&mut self) {
        let Some(transfer) = self.transfer.as_mut() else {
            return;
        };
        let completed_transfer_id = transfer.file.take().map(|_| transfer.transfer_id.clone());
        // A multi-file batch reuses the same engine/session but starts a
        // fresh UI row per file.
        let next_id = events::new_transfer_id();
        transfer.transfer_id = next_id.clone();
        if let Some(handle) = self.zmodem_map.write().unwrap().get_mut(&self.tab_id) {
            handle.transfer_id = next_id;
        }
        if let Some(transfer_id) = completed_transfer_id {
            self.emit_event(
                "transfer-complete",
                TransferCompletePayload {
                    transfer_id,
                    success: true,
                    cancelled: false,
                    error: None,
                },
            );
        }
    }

    fn on_session_ended(&mut self) {
        self.zmodem_map.write().unwrap().remove(&self.tab_id);
        self.transfer = None;
    }

    fn on_peer_cancelled(&mut self) {
        if let Some(transfer) = self.transfer.as_ref() {
            if transfer.file.is_some() {
                self.emit_event(
                    "transfer-complete",
                    TransferCompletePayload {
                        transfer_id: transfer.transfer_id.clone(),
                        success: false,
                        cancelled: true,
                        error: Some("Cancelled by the remote rz/sz process".to_string()),
                    },
                );
            }
        }
        self.zmodem_map.write().unwrap().remove(&self.tab_id);
        self.transfer = None;
    }

    fn on_local_cancel(&mut self) {
        if let Some(transfer) = self.transfer.as_ref() {
            if transfer.file.is_some() {
                self.emit_event(
                    "transfer-complete",
                    TransferCompletePayload {
                        transfer_id: transfer.transfer_id.clone(),
                        success: false,
                        cancelled: true,
                        error: None,
                    },
                );
            }
        }
        self.zmodem_map.write().unwrap().remove(&self.tab_id);
        self.transfer = None;
    }
}

/// Resolves the configured ZMODEM download directory, falling back to the
/// platform Downloads directory when unset (empty string).
pub fn resolve_download_dir(configured: &str) -> PathBuf {
    let trimmed = configured.trim();
    if !trimmed.is_empty() {
        return PathBuf::from(trimmed);
    }
    default_download_dir()
}

fn default_download_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let userprofile = userprofile.trim();
            if !userprofile.is_empty() {
                return PathBuf::from(userprofile).join("Downloads");
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let home = home.trim();
        if !home.is_empty() {
            return PathBuf::from(home).join("Downloads");
        }
    }
    std::env::temp_dir()
}

/// Picks a collision-safe destination path under `dir` for `name` (a
/// peer-supplied filename, so `file_name()` also strips any path component
/// to prevent it from escaping `dir` via `..`/absolute-path tricks).
fn unique_destination_path(dir: &Path, name: &str) -> PathBuf {
    let _ = std::fs::create_dir_all(dir);
    let safe_name = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("zmodem-received-file")
        .to_string();

    let candidate = dir.join(&safe_name);
    if !candidate.exists() {
        return candidate;
    }

    let (stem, ext) = match safe_name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), Some(ext.to_string())),
        _ => (safe_name.clone(), None),
    };
    for n in 1..10_000u32 {
        let candidate_name = match &ext {
            Some(ext) => format!("{stem} ({n}).{ext}"),
            None => format!("{stem} ({n})"),
        };
        let candidate = dir.join(candidate_name);
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(safe_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_destination_path_avoids_collisions() {
        let dir = std::env::temp_dir().join(format!("tterm-zmodem-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("file.bin"), b"a").unwrap();
        std::fs::write(dir.join("file (1).bin"), b"a").unwrap();

        let path = unique_destination_path(&dir, "file.bin");
        assert_eq!(path, dir.join("file (2).bin"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unique_destination_path_strips_directory_traversal() {
        let dir = std::env::temp_dir().join(format!("tterm-zmodem-test-{}", uuid::Uuid::new_v4()));
        let path = unique_destination_path(&dir, "../../etc/passwd");
        assert_eq!(path, dir.join("passwd"));
    }

    #[test]
    fn resolve_download_dir_uses_configured_value_when_set() {
        assert_eq!(
            resolve_download_dir("/tmp/custom"),
            PathBuf::from("/tmp/custom")
        );
    }

    #[test]
    fn resolve_download_dir_falls_back_when_empty() {
        let resolved = resolve_download_dir("  ");
        assert!(resolved.ends_with("Downloads") || resolved == std::env::temp_dir());
    }

    #[test]
    fn manual_override_forces_detection_on_when_auto_detect_is_disabled() {
        let zmodem_map: ZmodemMap =
            Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()));
        let trigger = super::super::protocol::consts::TRIGGER_ZRQINIT;
        let mut driver = ZmodemReceiveDriver::with_test_emit(
            |_, _| {},
            "manual-tab".to_string(),
            1,
            zmodem_map.clone(),
            false, // auto-detect disabled
            std::env::temp_dir(),
        );

        // Auto-detect is off and no manual arm yet: the trigger passes
        // through untouched, exactly like ordinary text.
        let outcome = driver.process(trigger);
        assert_eq!(outcome.passthrough, trigger);
        assert!(zmodem_map.read().unwrap().is_empty());

        // Manually arm (simulating the "Receive Files via ZMODEM" keymap
        // action): the same trigger now starts a receive session.
        driver.force_manual_override();
        let outcome = driver.process(trigger);
        assert!(outcome.passthrough.is_empty());
        assert!(zmodem_map.read().unwrap().contains_key("manual-tab"));
    }

    #[test]
    fn manual_override_is_one_shot_for_send_triggers() {
        let zmodem_map: ZmodemMap =
            Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()));
        let trigger = super::super::protocol::consts::TRIGGER_ZRINIT;
        let mut driver = ZmodemReceiveDriver::with_test_emit(
            |_, _| {},
            "manual-send-tab".to_string(),
            1,
            zmodem_map,
            false, // auto-detect disabled
            std::env::temp_dir(),
        );

        // Armed via "Send Files via ZMODEM": the `rz` trigger requests a send.
        driver.force_manual_override();
        let outcome = driver.process(trigger);
        assert!(outcome.send_requested.is_some());

        // The arm was consumed, so a later trigger is plain text again.
        let outcome = driver.process(trigger);
        assert!(outcome.send_requested.is_none());
        assert_eq!(outcome.passthrough, trigger);
    }
}
