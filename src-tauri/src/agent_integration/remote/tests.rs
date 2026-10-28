use std::fs;

use super::*;
use crate::sftp::internal::transfer::test_server::{ServerOptions, TestServer};

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

#[test]
fn reads_the_login_shells_variables_over_those_sh_got() {
    let output = "HOME=/home/me\nCODEX_HOME=/old\nPATH=/bin\n\
                  TTERM-AGENT-LOGIN-ENV\nWelcome!\nCODEX_HOME=/srv/codex\nX=1\n";
    assert_eq!(
        parse_env(output),
        env(&[("HOME", "/home/me"), ("CODEX_HOME", "/srv/codex")])
    );
}

#[test]
fn finds_each_config_directory() {
    let claude = Agent::ClaudeCode.spec();
    let opencode = Agent::OpenCode.spec();
    let home = env(&[("HOME", "/home/me/")]);
    assert_eq!(
        config_dir(&claude, &home),
        Some(("/home/me/.claude".to_string(), ConfigSource::Default))
    );
    assert_eq!(
        config_dir(&opencode, &home),
        Some((
            "/home/me/.config/opencode".to_string(),
            ConfigSource::Default
        ))
    );

    let moved = env(&[("HOME", "/home/me"), ("XDG_CONFIG_HOME", "/cfg/")]);
    assert_eq!(
        config_dir(&opencode, &moved),
        Some(("/cfg/opencode".to_string(), ConfigSource::Environment))
    );
    let tilde = env(&[
        ("HOME", "/home/me"),
        ("CLAUDE_CONFIG_DIR", "~/agents/claude"),
    ]);
    assert_eq!(
        config_dir(&claude, &tilde),
        Some((
            "/home/me/agents/claude".to_string(),
            ConfigSource::Environment
        ))
    );
    // A relative directory depends on where the agent starts.
    let relative = env(&[("HOME", "/home/me"), ("CLAUDE_CONFIG_DIR", "claude")]);
    assert_eq!(
        config_dir(&claude, &relative).unwrap().1,
        ConfigSource::Default
    );
    assert_eq!(config_dir(&claude, &env(&[])), None);
}

#[test]
fn hooks_on_a_host_never_run_this_computers_tterm() {
    for agent in AGENTS {
        if let Integration::Hooks(hooks) = agent.spec().for_remote().integration {
            assert!(hooks.command_windows.is_none());
        }
    }
}

#[tokio::test]
async fn installs_and_removes_hooks_beside_the_users_settings() {
    let server = TestServer::start(ServerOptions::default()).await;
    let sftp = server.sftp_session().await;
    let home = env(&[("HOME", "/home/me")]);
    let claude_dir = server.root().join("home/me/.claude");

    let before = status(&sftp, &home, Agent::ClaudeCode).await;
    assert_eq!(before.state, IntegrationState::NotInstalled);
    assert!(!before.detected);
    assert_eq!(before.config_path, "/home/me/.claude/settings.json");

    fs::create_dir_all(&claude_dir).unwrap();
    let original = "{\n  \"model\": \"opus\"\n}\n";
    fs::write(claude_dir.join("settings.json"), original).unwrap();

    let installed = change(&sftp, &home, Agent::ClaudeCode, true).await.unwrap();
    assert_eq!(installed.state, IntegrationState::Installed);
    assert!(installed.detected);
    let settings = fs::read_to_string(claude_dir.join("settings.json")).unwrap();
    assert!(settings.contains("]777;tterm-agent;claude;done"));
    assert!(settings.starts_with("{\n  \"model\": \"opus\","));
    assert_eq!(
        fs::read_to_string(claude_dir.join("settings.json.tterm-bak")).unwrap(),
        original
    );

    let removed = change(&sftp, &home, Agent::ClaudeCode, false)
        .await
        .unwrap();
    assert_eq!(removed.state, IntegrationState::NotInstalled);
    assert_eq!(
        fs::read_to_string(claude_dir.join("settings.json")).unwrap(),
        original
    );
}

#[tokio::test]
async fn writes_and_deletes_a_plugin_file_and_the_folders_it_needs() {
    let server = TestServer::start(ServerOptions::default()).await;
    let sftp = server.sftp_session().await;
    let home = env(&[("HOME", "/home/me")]);
    let plugins = server.root().join("home/me/.config/opencode/plugins");

    let installed = change(&sftp, &home, Agent::OpenCode, true).await.unwrap();
    assert_eq!(installed.state, IntegrationState::Installed);
    let Integration::Plugin(plugin) = Agent::OpenCode.spec().integration else {
        panic!("OpenCode takes a plugin file")
    };
    assert_eq!(
        fs::read_to_string(plugins.join("tterm-agent/tui.js")).unwrap(),
        plugin.content
    );

    let removed = change(&sftp, &home, Agent::OpenCode, false).await.unwrap();
    assert_eq!(removed.state, IntegrationState::NotInstalled);
    assert!(!plugins.join("tterm-agent").exists());
    assert!(plugins.exists());
}

#[tokio::test]
async fn leaves_settings_it_cannot_read_alone() {
    let server = TestServer::start(ServerOptions::default()).await;
    let sftp = server.sftp_session().await;
    let home = env(&[("HOME", "/home/me")]);
    let codex_dir = server.root().join("home/me/.codex");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::write(codex_dir.join("hooks.json"), "{ not json").unwrap();

    let current = status(&sftp, &home, Agent::Codex).await;
    assert!(current
        .error
        .is_some_and(|error| error.contains("hooks.json")));
    assert!(change(&sftp, &home, Agent::Codex, true).await.is_err());
    assert_eq!(
        fs::read_to_string(codex_dir.join("hooks.json")).unwrap(),
        "{ not json"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn writes_a_symlinked_settings_file_through_the_link() {
    let server = TestServer::start(ServerOptions::default()).await;
    let sftp = server.sftp_session().await;
    let home = env(&[("HOME", "/home/me")]);
    let dotfiles = server.root().join("home/me/dotfiles");
    let claude_dir = server.root().join("home/me/.claude");
    fs::create_dir_all(&dotfiles).unwrap();
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(dotfiles.join("claude.json"), "{}\n").unwrap();
    std::os::unix::fs::symlink(
        dotfiles.join("claude.json"),
        claude_dir.join("settings.json"),
    )
    .unwrap();

    change(&sftp, &home, Agent::ClaudeCode, true).await.unwrap();
    assert!(fs::symlink_metadata(claude_dir.join("settings.json"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(fs::read_to_string(dotfiles.join("claude.json"))
        .unwrap()
        .contains("tterm-agent"));
}

/// The environment as OpenSSH hands it to the login shell, which the
/// in-process server cannot stand in for. Reads only.
#[cfg(unix)]
#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_reads_the_home_directory_and_each_agents_place() {
    use crate::ssh::test_sshd::TestSshd;

    let sshd = TestSshd::spawn(12358, &["-t", "ed25519"], "Subsystem sftp internal-sftp\n").await;
    let (ssh, sftp) = sshd.sftp_session().await;
    let env = read_env(&ssh).await.expect("read the environment");
    let home = env.get("HOME").expect("HOME").clone();
    assert!(home.starts_with('/'), "{env:?}");

    let claude = status(&sftp, &env, Agent::ClaudeCode).await;
    assert!(claude.config_path.ends_with("/settings.json"), "{claude:?}");
    assert!(claude.config_dir.starts_with('/'), "{claude:?}");
}
