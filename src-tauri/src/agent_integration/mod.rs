//! Hooks and plugins tTerm installs into AI coding agents so they report what
//! they are doing (working, waiting for the user, done) to the terminal they
//! run in, as `OSC 777 ; tterm-agent ; <agent> ; <state> BEL`. The report
//! travels with the agent's own output, so it reaches tTerm from a local
//! shell and over SSH alike; terminals that do not know it ignore it.
//!
//! States: `processing`, `waiting` (for the user's permission or answer),
//! `done`, `error`, `idle` (stopped by the user, nothing to announce) and
//! `ended` (the agent quit).

mod claude_code;
mod codex;
mod hooks;
mod json;
mod login_shell;
mod plugin_file;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use hooks::HookSet;
use plugin_file::PluginFile;

/// Every hook command and plugin tTerm installs contains this, and nothing
/// else should.
const REPORT_MARKER: &str = "]777;tterm-agent;";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Agent {
    ClaudeCode,
    Codex,
    OpenCode,
    Pi,
}

const AGENTS: [Agent; 4] = [Agent::ClaudeCode, Agent::Codex, Agent::OpenCode, Agent::Pi];

/// How tTerm reaches into an agent.
enum Integration {
    /// Hooks merged into the agent's JSON settings file.
    Hooks(&'static HookSet),
    /// A file of tTerm's own the agent loads.
    Plugin(PluginFile),
}

/// Where an agent keeps its configuration and how tTerm hooks into it.
struct AgentSpec {
    /// The settings key for a config directory chosen by hand.
    key: &'static str,
    /// The variable that moves the config directory, and the path under it.
    variable: &'static str,
    under_variable: &'static [&'static str],
    /// The config directory under the home directory otherwise.
    default_dir: &'static [&'static str],
    integration: Integration,
}

impl Agent {
    fn spec(self) -> AgentSpec {
        match self {
            Agent::ClaudeCode => AgentSpec {
                key: "claudeCode",
                variable: "CLAUDE_CONFIG_DIR",
                under_variable: &[],
                default_dir: &[".claude"],
                integration: Integration::Hooks(&claude_code::HOOKS),
            },
            Agent::Codex => AgentSpec {
                key: "codex",
                variable: "CODEX_HOME",
                under_variable: &[],
                default_dir: &[".codex"],
                integration: Integration::Hooks(&codex::HOOKS),
            },
            // OpenCode 2 loads TUI plugins from folders under `plugins`;
            // OpenCode 1 only loads loose files there, so it skips this one.
            // It uses `~/.config` on every platform.
            Agent::OpenCode => AgentSpec {
                key: "openCode",
                variable: "XDG_CONFIG_HOME",
                under_variable: &["opencode"],
                default_dir: &[".config", "opencode"],
                integration: Integration::Plugin(PluginFile {
                    path: &["plugins", "tterm-agent", "tui.js"],
                    content: include_str!("opencode_tui.js"),
                }),
            },
            Agent::Pi => AgentSpec {
                key: "pi",
                variable: "PI_CODING_AGENT_DIR",
                under_variable: &[],
                default_dir: &[".pi", "agent"],
                integration: Integration::Plugin(PluginFile {
                    path: &["extensions", "tterm-agent.ts"],
                    content: include_str!("pi_extension.ts"),
                }),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IntegrationState {
    NotInstalled,
    Installed,
    /// Hooks from another tTerm version, or edited by hand.
    Outdated,
}

/// How tTerm found the agent's config directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigSource {
    /// Chosen in tTerm's settings.
    Setting,
    /// The agent's environment variable, set for tTerm or in the shell.
    Environment,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IntegrationKind {
    /// Merged into a settings file of the agent's.
    Hooks,
    /// A file of tTerm's own.
    Plugin,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentIntegrationStatus {
    agent: Agent,
    kind: IntegrationKind,
    state: IntegrationState,
    /// The agent's config directory exists: it has run on this computer.
    detected: bool,
    config_dir: String,
    /// The file tTerm changes.
    config_path: String,
    config_source: ConfigSource,
    /// What tTerm adds to that file.
    preview: String,
    /// Why the file could not be read.
    error: Option<String>,
}

/// The directory chosen in tTerm's settings, else the agent's variable from
/// tTerm's environment or the user's shell, else its default. The first two
/// must be absolute: a relative one depends on where the agent starts.
fn config_dir(spec: &AgentSpec) -> Option<(PathBuf, ConfigSource)> {
    let config = crate::config::load_config_file().unwrap_or_default();
    let custom = config
        .agent_config_dirs
        .get(spec.key)
        .map(|dir| crate::profiles::expand_home_path(dir))
        .filter(|dir| dir.is_absolute());
    if let Some(dir) = custom {
        return Some((dir, ConfigSource::Setting));
    }
    let from_environment = std::env::var(spec.variable)
        .ok()
        .filter(|dir| !dir.trim().is_empty())
        .or_else(|| login_shell::variable(spec.variable))
        .map(|dir| crate::profiles::expand_home_path(&dir))
        .filter(|dir| dir.is_absolute());
    if let Some(dir) = from_environment {
        return Some((join(dir, spec.under_variable), ConfigSource::Environment));
    }
    crate::profiles::home_dir().map(|home| (join(home, spec.default_dir), ConfigSource::Default))
}

fn join(base: PathBuf, parts: &[&str]) -> PathBuf {
    parts.iter().fold(base, |path, part| path.join(part))
}

fn target_path(spec: &AgentSpec, dir: &Path) -> PathBuf {
    match &spec.integration {
        Integration::Hooks(hooks) => dir.join(hooks.file),
        Integration::Plugin(plugin) => join(dir.to_path_buf(), plugin.path),
    }
}

fn read_state(spec: &AgentSpec, path: &Path) -> Result<IntegrationState, String> {
    match &spec.integration {
        Integration::Hooks(hooks) => json::read_object(path).map(|file| hooks.state(&file)),
        Integration::Plugin(plugin) => plugin.state(path),
    }
}

fn status(agent: Agent) -> AgentIntegrationStatus {
    let spec = agent.spec();
    let config_dir = config_dir(&spec);
    let path = config_dir.as_ref().map(|(dir, _)| target_path(&spec, dir));
    let (state, error) = match path.as_deref().map(|path| read_state(&spec, path)) {
        Some(Ok(state)) => (state, None),
        Some(Err(error)) => (IntegrationState::NotInstalled, Some(error)),
        None => (
            IntegrationState::NotInstalled,
            Some("No home directory".to_string()),
        ),
    };
    let (kind, preview) = match &spec.integration {
        Integration::Hooks(hooks) => (IntegrationKind::Hooks, json::to_text(&hooks.preview())),
        Integration::Plugin(plugin) => (IntegrationKind::Plugin, plugin.content.to_string()),
    };
    AgentIntegrationStatus {
        agent,
        kind,
        state,
        detected: config_dir.as_ref().is_some_and(|(dir, _)| dir.is_dir()),
        config_dir: config_dir
            .as_ref()
            .map(|(dir, _)| dir.display().to_string())
            .unwrap_or_default(),
        config_path: path
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        config_source: config_dir
            .map(|(_, source)| source)
            .unwrap_or(ConfigSource::Default),
        preview,
        error,
    }
}

fn change(agent: Agent, install: bool) -> Result<AgentIntegrationStatus, String> {
    let spec = agent.spec();
    let (dir, _) = config_dir(&spec).ok_or("No home directory")?;
    let path = target_path(&spec, &dir);
    match &spec.integration {
        Integration::Hooks(hooks) => {
            let current = json::read_object(&path)?;
            let mut settings = current.clone();
            if install {
                hooks.install(&mut settings)?;
            } else {
                hooks.uninstall(&mut settings);
            }
            // Removing nothing must not create the file.
            if settings != current {
                json::write_object(&path, &settings)?;
            }
        }
        Integration::Plugin(plugin) if install => plugin.install(&path)?,
        Integration::Plugin(plugin) => plugin.uninstall(&path)?,
    }
    Ok(status(agent))
}

/// Off the async runtime: the first lookup may start the user's shell.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn agent_integration_status() -> Result<Vec<AgentIntegrationStatus>, String> {
    blocking(|| AGENTS.into_iter().map(status).collect()).await
}

#[tauri::command]
pub async fn install_agent_integration(agent: Agent) -> Result<AgentIntegrationStatus, String> {
    blocking(move || change(agent, true)).await?
}

#[tauri::command]
pub async fn uninstall_agent_integration(agent: Agent) -> Result<AgentIntegrationStatus, String> {
    blocking(move || change(agent, false)).await?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_keys_match_the_agents_names() {
        for agent in AGENTS {
            let name = serde_json::to_value(agent).unwrap();
            assert_eq!(name.as_str(), Some(agent.spec().key));
        }
    }

    #[test]
    fn plugins_report_to_tterm() {
        for agent in AGENTS {
            if let Integration::Plugin(plugin) = agent.spec().integration {
                assert!(plugin.content.contains(REPORT_MARKER));
            }
        }
    }
}
