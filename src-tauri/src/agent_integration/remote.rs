//! Agent integration on an SSH host, for agents that run there: the same
//! hooks and plugin files as on this computer, read and written over the
//! tab's SFTP connection. A config directory comes from the host's
//! environment as an interactive login shell sets it up, where users export
//! such variables, or else from its default under the home directory.
//!
//! SFTP's rename will not replace an existing file on OpenSSH, so files are
//! rewritten in place, which also writes a symlinked settings file (dotfile
//! managers) through its link. A settings file's previous text is kept
//! beside it as `<name>.tterm-bak` first, as on this computer.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use russh::client::{Handle, Handler};
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::StatusCode;
use tauri::{AppHandle, State};
use tokio::io::AsyncWriteExt;

use super::json::{self, Object};
use super::{
    status_of, Agent, AgentIntegrationStatus, AgentSpec, ConfigSource, Integration,
    IntegrationState, Location, AGENTS,
};
use crate::core::session::types::PtyConnectionOptions;
use crate::core::state::HostPromptMap;
use crate::sftp::internal::api::SftpHost;
use crate::sftp::internal::exec::RemoteScript;
use crate::sftp::internal::types::SftpConnectionPool;
use crate::ssh::secret_store::types::SecretStoreState;

/// Bounds reading the host's environment: a login shell that takes longer
/// leaves the variables only it sets unread.
const ENV_TIMEOUT: Duration = Duration::from_secs(10);

/// Separates the environment `sh` got from the one the login shell set up.
const LOGIN_MARKER: &str = "TTERM-AGENT-LOGIN-ENV";

/// Prints the environment twice: as `sh` got it, then as the user's
/// interactive login shell sets it up. Its noise goes to /dev/null, and
/// shells that refuse `-l -i -c` (csh) leave the first.
const ENV_SCRIPT: &str = concat!(
    "env\n",
    "echo TTERM-AGENT-LOGIN-ENV\n",
    "\"${SHELL:-/bin/sh}\" -l -i -c env </dev/null 2>/dev/null\n",
);

const NO_HOME: &str = "The server did not say where its home directory is. \
                       tTerm asks with sh, which this server may not have.";

/// The variables in `env` output that the config directories depend on;
/// the login shell's values win over those `sh` got.
fn parse_env(output: &str) -> HashMap<String, String> {
    let wanted: Vec<&str> = AGENTS
        .iter()
        .map(|agent| agent.spec().variable)
        .chain(["HOME"])
        .collect();
    let mut env = HashMap::new();
    for line in output.lines() {
        if line.trim_end() == LOGIN_MARKER {
            continue;
        }
        if let Some((name, value)) = line.split_once('=') {
            if wanted.contains(&name) {
                env.insert(name.to_string(), value.to_string());
            }
        }
    }
    env
}

fn join(base: &str, parts: &[&str]) -> String {
    parts
        .iter()
        .fold(base.trim_end_matches('/').to_string(), |path, part| {
            format!("{path}/{part}")
        })
}

/// The agent's variable when it holds an absolute path (`~` is the home
/// directory), else its default under the home directory.
fn config_dir(spec: &AgentSpec, env: &HashMap<String, String>) -> Option<(String, ConfigSource)> {
    let home = env.get("HOME").filter(|home| home.starts_with('/'));
    let from_variable = env
        .get(spec.variable)
        .map(|dir| dir.trim())
        .and_then(|dir| match dir.strip_prefix('~') {
            Some(rest) if rest.is_empty() || rest.starts_with('/') => {
                home.map(|home| join(home, &[]) + rest)
            }
            _ => Some(dir.to_string()),
        })
        .filter(|dir| dir.starts_with('/'));
    if let Some(dir) = from_variable {
        return Some((join(&dir, spec.under_variable), ConfigSource::Environment));
    }
    home.map(|home| (join(home, spec.default_dir), ConfigSource::Default))
}

fn target_path(spec: &AgentSpec, dir: &str) -> String {
    match &spec.integration {
        Integration::Hooks(hooks) => join(dir, &[hooks.file]),
        Integration::Plugin(plugin) => join(dir, plugin.path),
    }
}

/// What the file at `path` holding `text` (none when missing) means.
fn state_of(spec: &AgentSpec, text: Option<&str>, path: &str) -> Result<IntegrationState, String> {
    match (&spec.integration, text) {
        (Integration::Hooks(hooks), _) => Ok(hooks.state(&settings_of(text, path)?)),
        (Integration::Plugin(_), None) => Ok(IntegrationState::NotInstalled),
        (Integration::Plugin(plugin), Some(text)) => plugin.state_of(text, path),
    }
}

/// A settings file's top-level object; an empty one when it is missing.
fn settings_of(text: Option<&str>, path: &str) -> Result<Object, String> {
    json::parse_object(text.unwrap_or_default()).map_err(|error| format!("{path}: {error}"))
}

/// Reads the variables the config directories depend on.
async fn read_env<H: Handler>(ssh: &Handle<H>) -> Result<HashMap<String, String>, String> {
    let mut script = RemoteScript::start(ssh, ENV_SCRIPT).await?;
    let deadline = tokio::time::Instant::now() + ENV_TIMEOUT;
    let mut output = Vec::new();
    loop {
        match tokio::time::timeout_at(deadline, script.next_output()).await {
            Ok(Some(chunk)) => output.extend_from_slice(&chunk),
            Ok(None) => {
                script.finish().await;
                break;
            }
            Err(_) => {
                script.stop().await;
                break;
            }
        }
    }
    Ok(parse_env(&String::from_utf8_lossy(&output)))
}

fn is_missing(error: &SftpError) -> bool {
    matches!(error, SftpError::Status(status) if status.status_code == StatusCode::NoSuchFile)
}

/// The file's text, or None when it does not exist.
async fn read_text(sftp: &SftpSession, path: &str) -> Result<Option<String>, String> {
    match sftp.read(path).await {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| format!("{path} is not UTF-8 text")),
        Err(error) if is_missing(&error) => Ok(None),
        Err(error) => Err(format!("Failed to read {path}: {error}")),
    }
}

async fn is_dir(sftp: &SftpSession, path: &str) -> bool {
    sftp.metadata(path)
        .await
        .is_ok_and(|metadata| metadata.is_dir())
}

/// Creates the folders above `path` that do not exist yet.
async fn create_parents(sftp: &SftpSession, path: &str) -> Result<(), String> {
    let mut missing = Vec::new();
    let mut dir = path.rsplit_once('/').map_or("", |(parent, _)| parent);
    while !dir.is_empty() {
        match sftp.try_exists(dir).await {
            Ok(true) => break,
            Ok(false) => missing.push(dir),
            Err(error) => return Err(format!("Failed to look up {dir}: {error}")),
        }
        dir = dir.rsplit_once('/').map_or("", |(parent, _)| parent);
    }
    for dir in missing.into_iter().rev() {
        sftp.create_dir(dir)
            .await
            .map_err(|error| format!("Failed to create {dir}: {error}"))?;
    }
    Ok(())
}

async fn write_text(sftp: &SftpSession, path: &str, text: &str) -> Result<(), String> {
    create_parents(sftp, path).await?;
    let mut file = sftp
        .create(path)
        .await
        .map_err(|error| format!("Failed to write {path}: {error}"))?;
    file.write_all(text.as_bytes())
        .await
        .map_err(|error| format!("Failed to write {path}: {error}"))?;
    file.shutdown()
        .await
        .map_err(|error| format!("Failed to write {path}: {error}"))
}

/// Writes a settings file, keeping its previous text as `<name>.tterm-bak`.
async fn write_settings(
    sftp: &SftpSession,
    path: &str,
    previous: Option<&str>,
    text: &str,
) -> Result<(), String> {
    if let Some(previous) = previous {
        write_text(sftp, &format!("{path}.tterm-bak"), previous).await?;
    }
    write_text(sftp, path, text).await
}

async fn status(
    sftp: &SftpSession,
    env: &HashMap<String, String>,
    agent: Agent,
) -> AgentIntegrationStatus {
    let spec = agent.spec().for_remote();
    let Some((dir, source)) = config_dir(&spec, env) else {
        return status_of(agent, &spec, None, Err(NO_HOME.to_string()), false);
    };
    let path = target_path(&spec, &dir);
    let state = match read_text(sftp, &path).await {
        Ok(text) => state_of(&spec, text.as_deref(), &path),
        Err(error) => Err(error),
    };
    let detected = is_dir(sftp, &dir).await;
    let location = Location { dir, path, source };
    status_of(agent, &spec, Some(location), state, detected)
}

async fn change(
    sftp: &SftpSession,
    env: &HashMap<String, String>,
    agent: Agent,
    install: bool,
) -> Result<AgentIntegrationStatus, String> {
    let spec = agent.spec().for_remote();
    let (dir, _) = config_dir(&spec, env).ok_or(NO_HOME)?;
    let path = target_path(&spec, &dir);
    let text = read_text(sftp, &path).await?;
    match &spec.integration {
        Integration::Hooks(hooks) => {
            let current = settings_of(text.as_deref(), &path)?;
            let mut settings = current.clone();
            if install {
                hooks.install(&mut settings)?;
            } else {
                hooks.uninstall(&mut settings);
            }
            // Removing nothing must not create the file.
            if settings != current {
                write_settings(sftp, &path, text.as_deref(), &json::to_text(&settings)).await?;
            }
        }
        Integration::Plugin(plugin) => {
            let state = state_of(&spec, text.as_deref(), &path)?;
            if install && state != IntegrationState::Installed {
                write_text(sftp, &path, plugin.content).await?;
            } else if !install && state != IntegrationState::NotInstalled {
                sftp.remove_file(path.as_str())
                    .await
                    .map_err(|error| format!("Failed to remove {path}: {error}"))?;
                // The folder it had to itself; fails, as it should, when
                // something else is in it.
                if plugin.path.len() > 2 {
                    if let Some((folder, _)) = path.rsplit_once('/') {
                        let _ = sftp.remove_dir(folder).await;
                    }
                }
            }
        }
    }
    Ok(status(sftp, env, agent).await)
}

/// The host's SFTP session and the environment its agents see.
async fn open(host: SftpHost<'_>) -> Result<(Arc<SftpSession>, HashMap<String, String>), String> {
    let (sftp, ssh) = host.handles().await?;
    let env = read_env(&ssh).await?;
    Ok((sftp, env))
}

/// Each agent's integration on the tab's SSH host.
#[tauri::command]
pub async fn remote_agent_integration_status(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<Vec<AgentIntegrationStatus>, String> {
    let (sftp, env) = open(SftpHost {
        app: &app,
        tab_id: &tab_id,
        connection,
        prompts: prompt_state.inner().clone(),
        secret_state: &secret_state,
        pool: pool_state.inner(),
    })
    .await?;
    let mut statuses = Vec::with_capacity(AGENTS.len());
    for agent in AGENTS {
        statuses.push(status(&sftp, &env, agent).await);
    }
    Ok(statuses)
}

/// Installs or removes `agent`'s integration on the tab's SSH host.
#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its IPC fields and injected state"
)]
pub async fn change_remote_agent_integration(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    agent: Agent,
    install: bool,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<AgentIntegrationStatus, String> {
    let (sftp, env) = open(SftpHost {
        app: &app,
        tab_id: &tab_id,
        connection,
        prompts: prompt_state.inner().clone(),
        secret_state: &secret_state,
        pool: pool_state.inner(),
    })
    .await?;
    change(&sftp, &env, agent, install).await
}

#[cfg(test)]
mod tests;
