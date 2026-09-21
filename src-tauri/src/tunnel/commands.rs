use super::credentials::{apply_credentials, collect_requests, StartOutcome, TunnelCredentials};
use super::runtime::{run_tunnel, RunContext, TunnelReporter};
use super::storage::{load_tunnels_from_disk, write_tunnels_to_disk};
use super::types::{TunnelRule, TunnelState, TunnelStatus};
use crate::core::session::{
    load_saved_jump_host_password, load_saved_ssh_password, normalize_connection,
    resolve_ssh_password, JumpHostOptions, PtyConnectionOptions,
};
use crate::core::state::HostPromptMap;
use crate::profiles::SavedProfile;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;

const STOP_TIMEOUT: Duration = Duration::from_secs(5);
/// Emitted when the app is asked to quit while tunnels are running.
pub const QUIT_REQUESTED_EVENT: &str = "tunnels-quit-requested";

struct RunningTunnel {
    stop_tx: watch::Sender<bool>,
    task: JoinHandle<()>,
}

#[derive(Default)]
pub struct TunnelManager {
    running: Mutex<HashMap<String, RunningTunnel>>,
    reporters: std::sync::Mutex<HashMap<String, Arc<TunnelReporter>>>,
    auto_started: AtomicBool,
    /// The user agreed to quit despite running tunnels.
    quit_confirmed: AtomicBool,
    /// A quit confirmation is on screen.
    quit_prompt_open: AtomicBool,
}

impl TunnelManager {
    fn reporter_for(&self, app: &AppHandle, id: &str) -> Arc<TunnelReporter> {
        self.reporters
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(id.to_string())
            .or_insert_with(|| TunnelReporter::new(app.clone(), id.to_string()))
            .clone()
    }

    fn status_of(&self, id: &str) -> TunnelStatus {
        self.reporters
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .map(|reporter| reporter.snapshot())
            .unwrap_or_else(|| TunnelStatus::stopped(id))
    }

    /// Stops every running tunnel that connects through `profile_id`.
    pub async fn stop_for_profile(&self, profile_id: &str) {
        let ids = load_tunnels_from_disk()
            .unwrap_or_default()
            .into_iter()
            .filter(|rule| rule.profile_id == profile_id)
            .map(|rule| rule.id)
            .collect::<Vec<_>>();
        for id in ids {
            self.stop(&id).await;
        }
    }

    fn is_running(&self, id: &str) -> bool {
        self.reporters
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .is_some_and(|reporter| reporter.state().is_active())
    }

    fn active_count(&self) -> usize {
        self.reporters
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .filter(|reporter| reporter.state().is_active())
            .count()
    }

    fn should_hold_exit(&self, active: usize) -> bool {
        if active == 0 || self.quit_confirmed.load(Ordering::SeqCst) {
            return false;
        }
        // Asking to quit again while the prompt is open means the user insists
        // (or the prompt could not be shown); never trap them in the app.
        !self.quit_prompt_open.swap(true, Ordering::SeqCst)
    }

    /// Called when the app is about to close. Returns `true` when the exit
    /// must wait because the UI is asking the user to confirm stopping the
    /// running tunnels.
    pub fn hold_exit_for_confirmation(&self, app: &AppHandle) -> bool {
        let active = self.active_count();
        let hold = self.should_hold_exit(active);
        if hold {
            let _ = app.emit(QUIT_REQUESTED_EVENT, active);
        }
        hold
    }

    async fn stop(&self, id: &str) {
        let running = self.running.lock().await.remove(id);
        let Some(running) = running else {
            return;
        };
        let _ = running.stop_tx.send(true);
        let mut task = running.task;
        if tokio::time::timeout(STOP_TIMEOUT, &mut task).await.is_err() {
            task.abort();
            if let Some(reporter) = self
                .reporters
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(id)
            {
                reporter.set_state(TunnelState::Stopped, None);
            }
        }
    }
}

/// Builds the connection a tunnel dials from the saved profile it points at.
/// Secrets are left unset; `resolve_ssh_password` fills them from the store.
fn connection_options_for_profile(profile: &SavedProfile) -> Result<PtyConnectionOptions, String> {
    if profile.connection_type != "ssh" {
        return Err(format!("'{}' is not an SSH connection", profile.name));
    }
    let uses_key = profile.auth_method.as_deref() == Some("key");
    let jump_hosts = if profile.uses_jump_host() {
        profile
            .jump_hosts
            .iter()
            .map(|jump| JumpHostOptions {
                host: Some(jump.host.clone()),
                port: Some(jump.port),
                username: Some(jump.username.clone()),
                password: None,
                auth_method: Some(jump.auth_method.clone()),
                private_key_path: (jump.auth_method == "key")
                    .then(|| jump.private_key_path.clone())
                    .flatten(),
                private_key_passphrase: None,
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(PtyConnectionOptions {
        connection_type: Some("ssh".to_string()),
        profile_id: Some(profile.id.clone()),
        profile_name: Some(profile.name.clone()),
        host: profile.host.clone(),
        port: profile.port,
        username: profile.username.clone(),
        keepalive_interval_secs: Some(profile.keepalive_interval_secs.min(u16::MAX as u32) as u16),
        keepalive_count_max: Some(profile.keepalive_count_max.min(u16::MAX as u32) as u16),
        private_key_path: uses_key.then(|| profile.private_key_path.clone()).flatten(),
        jump_hosts,
        ..PtyConnectionOptions::default()
    })
}

fn find_profile(profile_id: &str) -> Result<SavedProfile, String> {
    let mut profile = crate::profiles::load_profiles_from_disk()?
        .into_iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "The host for this tunnel no longer exists".to_string())?;
    crate::profiles::normalize_profile(&mut profile);
    Ok(profile)
}

#[tauri::command]
pub fn list_tunnels() -> Result<Vec<TunnelRule>, String> {
    load_tunnels_from_disk()
}

#[tauri::command]
pub fn list_tunnel_statuses(
    manager: State<'_, TunnelManager>,
) -> Result<Vec<TunnelStatus>, String> {
    Ok(load_tunnels_from_disk()?
        .iter()
        .map(|rule| manager.status_of(&rule.id))
        .collect())
}

#[tauri::command]
pub fn save_tunnel(mut tunnel: TunnelRule) -> Result<TunnelRule, String> {
    tunnel.validate()?;
    let profile = find_profile(&tunnel.profile_id)?;
    connection_options_for_profile(&profile)?;

    let mut tunnels = load_tunnels_from_disk()?;
    match tunnels.iter_mut().find(|existing| existing.id == tunnel.id) {
        Some(existing) => *existing = tunnel.clone(),
        None => tunnels.push(tunnel.clone()),
    }
    write_tunnels_to_disk(&tunnels)?;
    Ok(tunnel)
}

#[tauri::command]
pub async fn delete_tunnel(id: String, manager: State<'_, TunnelManager>) -> Result<(), String> {
    manager.stop(&id).await;
    manager
        .reporters
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id);
    let mut tunnels = load_tunnels_from_disk()?;
    tunnels.retain(|tunnel| tunnel.id != id);
    write_tunnels_to_disk(&tunnels)
}

/// Resolves a rule's connection, asking for missing secrets instead of
/// failing, and spawns its task. `auto` marks an unattended start: missing
/// secrets are recorded on the tunnel's status rather than prompted for.
async fn start_rule(
    app: &AppHandle,
    manager: &TunnelManager,
    secret_state: &crate::ssh::SecretStoreState,
    prompts: &HostPromptMap,
    id: &str,
    credentials: &TunnelCredentials,
    auto: bool,
) -> Result<StartOutcome, String> {
    let all_rules = load_tunnels_from_disk()?;
    let rule = all_rules
        .iter()
        .find(|rule| rule.id == id)
        .cloned()
        .ok_or_else(|| "Tunnel not found".to_string())?;
    // Two rules may share a port so long as only one runs at a time; say so
    // plainly instead of failing later with a bare "address in use".
    if let Some(rival) = all_rules.iter().find(|other| {
        other.id != id && other.competes_for_port_with(&rule) && manager.is_running(&other.id)
    }) {
        return Err(format!(
            "Port {} is already used by the running tunnel \"{}\". Stop it first.",
            rule.bind_port, rival.name
        ));
    }
    let profile = find_profile(&rule.profile_id)?;
    let mut options = connection_options_for_profile(&profile)?;
    apply_credentials(&mut options, credentials);
    let mut plan = normalize_connection(Some(options))?;

    let requests = collect_requests(
        &mut plan,
        |plan| {
            load_saved_ssh_password(
                app,
                secret_state,
                plan.profile_id.as_deref(),
                Some(plan.profile_name.as_str()),
            )
        },
        |plan, jump| {
            load_saved_jump_host_password(
                app,
                secret_state,
                plan.profile_id.as_deref(),
                Some(plan.profile_name.as_str()),
                Some(jump.host.as_str()),
                Some(jump.port),
                Some(jump.username.as_str()),
                plan.jump_hosts.len() == 1,
            )
        },
    )?;
    if !requests.is_empty() {
        if auto {
            manager
                .reporter_for(app, id)
                .set_state(TunnelState::NeedsCredentials, None);
        }
        return Ok(StartOutcome::NeedsCredentials { requests });
    }
    // Persists entered passwords when the user asked to remember them.
    resolve_ssh_password(app, secret_state, &mut plan)?;

    // Check and register under one lock so a double click cannot start two
    // tasks for the same rule.
    let mut running = manager.running.lock().await;
    if running
        .get(id)
        .is_some_and(|existing| !existing.task.is_finished())
    {
        return Ok(StartOutcome::Started);
    }

    let reporter = manager.reporter_for(app, id);
    let (stop_tx, stop_rx) = watch::channel(false);
    let task = tokio::spawn(run_tunnel(
        RunContext {
            app: app.clone(),
            rule,
            plan,
            prompts: prompts.clone(),
            reporter,
        },
        stop_rx,
    ));
    running.insert(id.to_string(), RunningTunnel { stop_tx, task });
    Ok(StartOutcome::Started)
}

#[tauri::command]
pub async fn start_tunnel(
    app: AppHandle,
    id: String,
    credentials: Option<TunnelCredentials>,
    manager: State<'_, TunnelManager>,
    secret_state: State<'_, crate::ssh::SecretStoreState>,
    prompts: State<'_, HostPromptMap>,
) -> Result<StartOutcome, String> {
    start_rule(
        &app,
        &manager,
        &secret_state,
        prompts.inner(),
        &id,
        &credentials.unwrap_or_default(),
        false,
    )
    .await
}

/// Starts every rule flagged `auto_start`. Runs once per app launch so that
/// reloading the window does not revive tunnels the user stopped.
#[tauri::command]
pub async fn auto_start_tunnels(
    app: AppHandle,
    manager: State<'_, TunnelManager>,
    secret_state: State<'_, crate::ssh::SecretStoreState>,
    prompts: State<'_, HostPromptMap>,
) -> Result<(), String> {
    if manager.auto_started.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    for rule in load_tunnels_from_disk()?
        .into_iter()
        .filter(|rule| rule.auto_start)
    {
        let result = start_rule(
            &app,
            &manager,
            &secret_state,
            prompts.inner(),
            &rule.id,
            &TunnelCredentials::default(),
            true,
        )
        .await;
        if let Err(message) = result {
            manager
                .reporter_for(&app, &rule.id)
                .set_state(TunnelState::Error, Some(message));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn confirm_quit_app(app: AppHandle, manager: State<'_, TunnelManager>) {
    manager.quit_confirmed.store(true, Ordering::SeqCst);
    app.exit(0);
}

#[tauri::command]
pub fn cancel_quit_prompt(manager: State<'_, TunnelManager>) {
    manager.quit_prompt_open.store(false, Ordering::SeqCst);
}

#[tauri::command]
pub async fn stop_tunnel(id: String, manager: State<'_, TunnelManager>) -> Result<(), String> {
    manager.stop(&id).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(json: serde_json::Value) -> SavedProfile {
        serde_json::from_value(json).expect("profile should deserialize")
    }

    #[test]
    fn exit_is_held_once_while_tunnels_run() {
        let manager = TunnelManager::default();
        assert!(!manager.should_hold_exit(0), "nothing running");

        assert!(manager.should_hold_exit(2), "first attempt asks the user");
        assert!(!manager.should_hold_exit(2), "insisting lets the app quit");

        manager.quit_prompt_open.store(false, Ordering::SeqCst);
        assert!(manager.should_hold_exit(2), "cancelling re-arms the prompt");

        manager.quit_confirmed.store(true, Ordering::SeqCst);
        assert!(!manager.should_hold_exit(2), "confirmed");
    }

    #[test]
    fn options_carry_key_auth_and_enabled_jump_hosts() {
        let profile = profile(serde_json::json!({
            "id": "p1", "name": "prod", "connection_type": "ssh",
            "host": "10.0.0.5", "port": 2222, "username": "deploy",
            "auth_method": "key", "private_key_path": "/keys/id",
            "use_jump_host": true,
            "jump_hosts": [{"host": "bastion", "port": 22, "username": "ops", "auth_method": "password"}]
        }));
        let options = connection_options_for_profile(&profile).unwrap();
        assert_eq!(options.host.as_deref(), Some("10.0.0.5"));
        assert_eq!(options.port, Some(2222));
        assert_eq!(options.private_key_path.as_deref(), Some("/keys/id"));
        assert_eq!(options.jump_hosts.len(), 1);
        assert_eq!(options.jump_hosts[0].host.as_deref(), Some("bastion"));
    }

    #[test]
    fn options_skip_a_disabled_jump_chain_and_password_profiles_carry_no_key() {
        let profile = profile(serde_json::json!({
            "id": "p1", "name": "prod", "connection_type": "ssh",
            "host": "h", "username": "u", "auth_method": "password",
            "private_key_path": "/stale/key",
            "use_jump_host": false,
            "jump_hosts": [{"host": "bastion", "port": 22, "username": "ops"}]
        }));
        let options = connection_options_for_profile(&profile).unwrap();
        assert!(options.jump_hosts.is_empty());
        assert!(options.private_key_path.is_none());
    }

    #[test]
    fn non_ssh_profiles_are_rejected() {
        let profile = profile(serde_json::json!({
            "id": "p1", "name": "local", "connection_type": "terminal"
        }));
        assert!(connection_options_for_profile(&profile).is_err());
    }
}
