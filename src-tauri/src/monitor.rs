use crate::core::session::{
    normalize_connection, resolve_ssh_password, PtyConnectionOptions, SessionPlan,
};
use crate::core::state::{HostPromptMap, SessionKind};
use crate::ssh::{
    open_target_ssh_session, ConnectionStatusOptions, JumpChain, SecretStoreState, SshClientHandler,
};
use russh::ChannelMsg;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration, Instant};

const MONITOR_SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

const METRICS_SCRIPT: &str = r#"    set -efu

    echo "__TTERM_OS_RELEASE_BEGIN__"
    if [ -r /etc/os-release ]; then
      cat /etc/os-release
    fi
    echo "__TTERM_OS_RELEASE_END__"

    printf "__TTERM_UNAME__ "
    uname -srm 2>/dev/null || true

    if [ "$(uname -s 2>/dev/null || true)" != "Linux" ]; then
      exit 0
    fi

    printf "__TTERM_PROC_STAT__ "
    awk '
      /^cpu / {
        print $2, $3, $4, $5, $6, $7, $8, $9, $10, $11
      }
    ' /proc/stat 2>/dev/null || true

    printf "__TTERM_PROC_STAT_CORES__ "
    awk '
      /^cpu[0-9]+ / {
        times = $2 "," $3 "," $4 "," $5 "," $6 "," $7 "," $8 "," $9 "," $10 "," $11
        printf "%s=%s ", $1, times
      }
      END {
        printf "\n"
      }
    ' /proc/stat 2>/dev/null || true

    printf "__TTERM_MEMINFO__ "
    awk '
      /^(MemTotal|MemAvailable|MemFree|Buffers|Cached|SReclaimable):/ {
        gsub(":", "", $1)
        printf "%s=%s ", $1, $2
      }
      END {
        printf "\n"
      }
    ' /proc/meminfo 2>/dev/null || true

    printf "__TTERM_PRIMARY_IP__ "
    (
      ip -o -4 route get 1.1.1.1 2>/dev/null | awk '
        {
          for (i = 1; i <= NF; i++) {
            if ($i == "src") {
              print $(i + 1)
              exit
            }
          }
        }
      '

      hostname -I 2>/dev/null | awk '
        {
          for (i = 1; i <= NF; i++) {
            if ($i !~ /^127\./ && $i !~ /^169\.254\./) {
              print $i
              exit
            }
          }
        }
      '

      ip -o -4 addr show scope global 2>/dev/null | awk '
        {
          split($4, parts, "/")
          print parts[1]
          exit
        }
      '
    ) | awk 'NF { print; exit }'
    printf "\n"

    printf "__TTERM_DF_ROOT__ "
    df -P -k / 2>/dev/null | awk 'NR == 2 { print $2, $3, $4, $5, $6 }' || true
"#;

pub type MonitorSessionMap = Arc<Mutex<HashMap<String, Arc<Mutex<MonitorSession>>>>>;

pub struct MonitorSession {
    session_nonce: u32,
    fingerprint: String,
    last_used_at: Instant,
    jump_chain: Option<JumpChain>,
    ssh: russh::client::Handle<SshClientHandler>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinuxDistributionInfo {
    pub id: String,
    pub id_like: Vec<String>,
    pub name: String,
    pub pretty_name: String,
    pub version_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuTimes {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
    pub guest: u64,
    pub guest_nice: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuCoreTimes {
    pub id: String,
    pub times: CpuTimes,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryMetrics {
    pub total_kib: u64,
    pub available_kib: u64,
    pub used_kib: u64,
    pub used_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskMetrics {
    pub mount: String,
    pub total_kib: u64,
    pub used_kib: u64,
    pub available_kib: u64,
    pub used_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerMetricsSnapshot {
    pub supported: bool,
    pub unsupported_reason: Option<String>,
    pub distribution: Option<LinuxDistributionInfo>,
    pub kernel: Option<String>,
    pub cpu_times: Option<CpuTimes>,
    pub cpu_core_times: Vec<CpuCoreTimes>,
    pub memory: Option<MemoryMetrics>,
    pub primary_ip: Option<String>,
    pub disk: Option<DiskMetrics>,
}

#[tauri::command]
pub async fn get_server_metrics_snapshot(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    connection: Option<PtyConnectionOptions>,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    monitor_sessions: State<'_, MonitorSessionMap>,
) -> Result<ServerMetricsSnapshot, String> {
    let mut plan = normalize_connection(connection)?;
    if !matches!(plan.kind, SessionKind::Ssh) {
        return Err("Server monitoring requires an SSH connection".to_string());
    }
    resolve_ssh_password(&app, &secret_state, &mut plan)?;
    reap_idle_monitor_sessions(monitor_sessions.inner().clone()).await;

    let fingerprint = monitor_connection_fingerprint(&plan)?;
    let monitor_session = get_or_open_monitor_session(
        &app,
        &tab_id,
        session_nonce,
        &plan,
        &fingerprint,
        prompt_state.inner().clone(),
        monitor_sessions.inner().clone(),
    )
    .await?;

    match collect_metrics_snapshot(monitor_session).await {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => {
            release_monitor_session_inner(
                monitor_sessions.inner().clone(),
                &tab_id,
                Some(session_nonce),
                "monitor metrics failed",
            )
            .await;
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn release_server_monitor_session(
    tab_id: String,
    session_nonce: Option<u32>,
    monitor_sessions: State<'_, MonitorSessionMap>,
) -> Result<(), String> {
    reap_idle_monitor_sessions(monitor_sessions.inner().clone()).await;
    release_monitor_session_inner(
        monitor_sessions.inner().clone(),
        &tab_id,
        session_nonce,
        "monitor session released",
    )
    .await;
    Ok(())
}

fn monitor_connection_fingerprint(plan: &SessionPlan) -> Result<String, String> {
    let host = plan
        .host
        .as_deref()
        .ok_or_else(|| "SSH host is required".to_string())?;
    let username = plan
        .username
        .as_deref()
        .ok_or_else(|| "SSH username is required".to_string())?;

    let jump_chain = plan
        .jump_hosts
        .iter()
        .map(|jump| {
            (
                jump.host.as_str(),
                jump.port,
                jump.username.as_str(),
                jump.private_key_path.as_deref().unwrap_or(""),
                jump.private_key_passphrase.is_some(),
            )
        })
        .collect::<Vec<_>>();

    serde_json::to_string(&(
        plan.profile_id.as_deref().unwrap_or(""),
        plan.profile_name.as_str(),
        host,
        plan.port,
        username,
        plan.private_key_path.as_deref().unwrap_or(""),
        plan.private_key_passphrase.is_some(),
        plan.keepalive_interval_secs,
        jump_chain,
    ))
    .map_err(|err| format!("Failed to fingerprint monitor connection: {err}"))
}

async fn get_or_open_monitor_session(
    app: &AppHandle,
    tab_id: &str,
    session_nonce: u32,
    plan: &SessionPlan,
    fingerprint: &str,
    prompts: HostPromptMap,
    monitor_sessions: MonitorSessionMap,
) -> Result<Arc<Mutex<MonitorSession>>, String> {
    let existing_session = {
        let sessions = monitor_sessions.lock().await;
        sessions.get(tab_id).cloned()
    };

    if let Some(session) = existing_session {
        let matches_request = {
            let mut session_guard = session.lock().await;
            session_guard.last_used_at = Instant::now();
            session_guard.session_nonce == session_nonce && session_guard.fingerprint == fingerprint
        };
        if matches_request {
            return Ok(session);
        }
    }

    let stale_session = {
        let mut sessions = monitor_sessions.lock().await;
        sessions.remove(tab_id)
    };
    close_monitor_session(stale_session, "monitor session replaced").await;

    let host = plan
        .host
        .clone()
        .ok_or_else(|| "SSH host is required".to_string())?;
    let username = plan
        .username
        .clone()
        .ok_or_else(|| "SSH username is required".to_string())?;

    let (jump_chain, ssh) = open_target_ssh_session(
        &app,
        tab_id,
        plan.profile_id.as_deref(),
        &plan.profile_name,
        &host,
        plan.port,
        &username,
        plan.private_key_path.as_deref(),
        plan.private_key_passphrase.as_deref(),
        plan.password.as_deref(),
        plan.keepalive_interval_secs,
        plan.keepalive_count_max,
        &plan.jump_hosts,
        prompts,
        ConnectionStatusOptions::QUIET,
        crate::ssh::HostKeyVerificationMode::PromptAndPersist,
    )
    .await?;

    let cached_session = Arc::new(Mutex::new(MonitorSession {
        session_nonce,
        fingerprint: fingerprint.to_string(),
        last_used_at: Instant::now(),
        jump_chain,
        ssh,
    }));

    let replaced_session = {
        let mut sessions = monitor_sessions.lock().await;
        sessions.insert(tab_id.to_string(), cached_session.clone())
    };
    close_monitor_session(replaced_session, "monitor session replaced").await;
    schedule_monitor_session_idle_timeout(
        monitor_sessions.clone(),
        tab_id.to_string(),
        cached_session.clone(),
    );

    Ok(cached_session)
}

async fn collect_metrics_snapshot(
    monitor_session: Arc<Mutex<MonitorSession>>,
) -> Result<ServerMetricsSnapshot, String> {
    let mut session = monitor_session.lock().await;
    session.last_used_at = Instant::now();
    let mut channel = session
        .ssh
        .channel_open_session()
        .await
        .map_err(|err| format!("Failed to open SSH channel: {err}"))?;

    channel
        .exec(true, METRICS_SCRIPT)
        .await
        .map_err(|err| format!("Failed to execute metrics command: {err}"))?;

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_status = None;
    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Data { data } => stdout.push_str(&String::from_utf8_lossy(&data)),
            ChannelMsg::ExtendedData { data, .. } => {
                stderr.push_str(&String::from_utf8_lossy(&data))
            }
            ChannelMsg::ExitStatus { exit_status: code } => exit_status = Some(code),
            ChannelMsg::Close => break,
            _ => {}
        }
    }

    let _ = channel.close().await;
    session.last_used_at = Instant::now();

    if exit_status.unwrap_or(0) != 0 {
        let detail = stderr.trim();
        return Err(if detail.is_empty() {
            format!(
                "Metrics command failed with exit status {}",
                exit_status.unwrap_or(1)
            )
        } else {
            format!(
                "Metrics command failed with exit status {}: {}",
                exit_status.unwrap_or(1),
                detail
            )
        });
    }

    Ok(parse_metrics_output(&stdout))
}

async fn reap_idle_monitor_sessions(monitor_sessions: MonitorSessionMap) {
    let sessions = {
        let sessions = monitor_sessions.lock().await;
        sessions
            .iter()
            .map(|(tab_id, session)| (tab_id.clone(), session.clone()))
            .collect::<Vec<_>>()
    };
    let now = Instant::now();
    let mut idle_sessions = Vec::new();

    for (tab_id, session) in sessions {
        let is_idle = {
            let session = session.lock().await;
            now.duration_since(session.last_used_at) >= MONITOR_SESSION_IDLE_TIMEOUT
        };
        if is_idle {
            idle_sessions.push((tab_id, session));
        }
    }

    for (tab_id, idle_session) in idle_sessions {
        let session_to_close = {
            let mut sessions = monitor_sessions.lock().await;
            let should_remove = sessions
                .get(&tab_id)
                .is_some_and(|current| Arc::ptr_eq(current, &idle_session));
            should_remove.then(|| sessions.remove(&tab_id)).flatten()
        };
        close_monitor_session(session_to_close, "monitor session idle timeout").await;
    }
}

fn schedule_monitor_session_idle_timeout(
    monitor_sessions: MonitorSessionMap,
    tab_id: String,
    watched_session: Arc<Mutex<MonitorSession>>,
) {
    tokio::spawn(async move {
        loop {
            sleep(MONITOR_SESSION_IDLE_TIMEOUT).await;

            let is_still_current = {
                let sessions = monitor_sessions.lock().await;
                sessions
                    .get(&tab_id)
                    .is_some_and(|current| Arc::ptr_eq(current, &watched_session))
            };
            if !is_still_current {
                break;
            }

            let is_idle = {
                let session = watched_session.lock().await;
                Instant::now().duration_since(session.last_used_at) >= MONITOR_SESSION_IDLE_TIMEOUT
            };
            if !is_idle {
                continue;
            }

            let session_to_close = {
                let mut sessions = monitor_sessions.lock().await;
                let should_remove = sessions
                    .get(&tab_id)
                    .is_some_and(|current| Arc::ptr_eq(current, &watched_session));
                should_remove.then(|| sessions.remove(&tab_id)).flatten()
            };
            close_monitor_session(session_to_close, "monitor session idle timeout").await;
            break;
        }
    });
}

async fn release_monitor_session_inner(
    monitor_sessions: MonitorSessionMap,
    tab_id: &str,
    session_nonce: Option<u32>,
    reason: &'static str,
) {
    let session = {
        let mut sessions = monitor_sessions.lock().await;
        sessions.remove(tab_id)
    };

    if let Some(session) = session {
        if let Some(nonce) = session_nonce {
            let matches_nonce = {
                let session_guard = session.lock().await;
                session_guard.session_nonce == nonce
            };
            if !matches_nonce {
                let mut session_to_close = None;
                let mut sessions = monitor_sessions.lock().await;
                if sessions.contains_key(tab_id) {
                    session_to_close = Some(session);
                } else {
                    sessions.insert(tab_id.to_string(), session);
                }
                drop(sessions);
                close_monitor_session(session_to_close, "monitor session superseded").await;
                return;
            }
        }

        close_monitor_session(Some(session), reason).await;
    }
}

async fn close_monitor_session(session: Option<Arc<Mutex<MonitorSession>>>, reason: &'static str) {
    if let Some(session) = session {
        let mut session = session.lock().await;
        if let Err(err) = session
            .ssh
            .disconnect(russh::Disconnect::ByApplication, reason, "en")
            .await
        {
            eprintln!("Failed to disconnect monitor session ({reason}): {err}");
        }
        drop(session.jump_chain.take());
    }
}

fn parse_metrics_output(output: &str) -> ServerMetricsSnapshot {
    let os_release = extract_block(
        output,
        "__TTERM_OS_RELEASE_BEGIN__",
        "__TTERM_OS_RELEASE_END__",
    );
    let uname = find_prefixed_line(output, "__TTERM_UNAME__").unwrap_or_default();
    let kernel = (!uname.trim().is_empty()).then(|| uname.trim().to_string());
    let supported = uname.split_whitespace().next() == Some("Linux");

    ServerMetricsSnapshot {
        supported,
        unsupported_reason: if supported {
            None
        } else {
            Some("Only Linux hosts are supported".to_string())
        },
        distribution: parse_os_release(os_release.unwrap_or_default()),
        kernel,
        cpu_times: find_prefixed_line(output, "__TTERM_PROC_STAT__")
            .and_then(|line| parse_cpu_times(&line)),
        cpu_core_times: find_prefixed_line(output, "__TTERM_PROC_STAT_CORES__")
            .map(|line| parse_cpu_core_times(&line))
            .unwrap_or_default(),
        memory: find_prefixed_line(output, "__TTERM_MEMINFO__")
            .and_then(|line| parse_memory_metrics(&line)),
        primary_ip: find_prefixed_line(output, "__TTERM_PRIMARY_IP__")
            .and_then(|line| parse_primary_ip(&line)),
        disk: find_prefixed_line(output, "__TTERM_DF_ROOT__")
            .and_then(|line| parse_disk_metrics(&line)),
    }
}

fn extract_block<'a>(output: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_idx = output.find(start)? + start.len();
    let rest = &output[start_idx..];
    let end_idx = rest.find(end)?;
    Some(rest[..end_idx].trim())
}

fn find_prefixed_line(output: &str, prefix: &str) -> Option<String> {
    output.lines().find_map(|line| {
        line.strip_prefix(prefix)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn parse_os_release(content: &str) -> Option<LinuxDistributionInfo> {
    if content.trim().is_empty() {
        return None;
    }

    let mut id = String::new();
    let mut id_like = Vec::new();
    let mut name = String::new();
    let mut pretty_name = String::new();
    let mut version_id = None;

    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = unquote_os_release_value(value.trim());
        match key.trim() {
            "ID" => id = value,
            "ID_LIKE" => {
                id_like = value.split_whitespace().map(str::to_string).collect();
            }
            "NAME" => name = value,
            "PRETTY_NAME" => pretty_name = value,
            "VERSION_ID" => version_id = Some(value),
            _ => {}
        }
    }

    if id.is_empty() && name.is_empty() && pretty_name.is_empty() {
        return None;
    }

    if pretty_name.is_empty() {
        pretty_name = name.clone();
    }
    if name.is_empty() {
        name = pretty_name.clone();
    }

    Some(LinuxDistributionInfo {
        id,
        id_like,
        name,
        pretty_name,
        version_id,
    })
}

fn unquote_os_release_value(value: &str) -> String {
    let unquoted = value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|inner| inner.strip_suffix('\''))
        })
        .unwrap_or(value);
    unquoted
        .replace("\\\"", "\"")
        .replace("\\'", "'")
        .replace("\\\\", "\\")
}

fn parse_cpu_times(line: &str) -> Option<CpuTimes> {
    let values = line
        .split_whitespace()
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if values.len() < 10 {
        return None;
    }

    Some(CpuTimes {
        user: values[0],
        nice: values[1],
        system: values[2],
        idle: values[3],
        iowait: values[4],
        irq: values[5],
        softirq: values[6],
        steal: values[7],
        guest: values[8],
        guest_nice: values[9],
    })
}

fn parse_cpu_core_times(line: &str) -> Vec<CpuCoreTimes> {
    line.split_whitespace()
        .filter_map(|item| {
            let (id, raw_times) = item.split_once('=')?;
            let times = parse_cpu_times(&raw_times.replace(',', " "))?;
            Some(CpuCoreTimes {
                id: id.to_string(),
                times,
            })
        })
        .collect()
}

fn parse_memory_metrics(line: &str) -> Option<MemoryMetrics> {
    let mut total = None;
    let mut available = None;
    let mut free = None;
    let mut buffers = None;
    let mut cached = None;
    let mut reclaimable = None;

    for item in line.split_whitespace() {
        let Some((key, raw_value)) = item.split_once('=') else {
            continue;
        };
        let value = raw_value.parse::<u64>().ok();
        match key {
            "MemTotal" => total = value,
            "MemAvailable" => available = value,
            "MemFree" => free = value,
            "Buffers" => buffers = value,
            "Cached" => cached = value,
            "SReclaimable" => reclaimable = value,
            _ => {}
        }
    }

    let total = total?;
    let available = available.or_else(|| {
        Some(
            free.unwrap_or(0)
                + buffers.unwrap_or(0)
                + cached.unwrap_or(0)
                + reclaimable.unwrap_or(0),
        )
    })?;
    let used = total.saturating_sub(available);
    let used_percent = percent(used, total);

    Some(MemoryMetrics {
        total_kib: total,
        available_kib: available,
        used_kib: used,
        used_percent,
    })
}

fn parse_primary_ip(line: &str) -> Option<String> {
    line.split_whitespace()
        .find(|value| !value.starts_with("127.") && !value.starts_with("169.254."))
        .map(str::to_string)
}

fn parse_disk_metrics(line: &str) -> Option<DiskMetrics> {
    let mut values = line.split_whitespace();
    let total = values.next()?.parse::<u64>().ok()?;
    let used = values.next()?.parse::<u64>().ok()?;
    let available = values.next()?.parse::<u64>().ok()?;
    let raw_percent = values.next().unwrap_or("0").trim_end_matches('%');
    let mount = values.next().unwrap_or("/").to_string();
    let used_percent = raw_percent
        .parse::<f64>()
        .unwrap_or_else(|_| percent(used, total));

    Some(DiskMetrics {
        mount,
        total_kib: total,
        used_kib: used,
        available_kib: available,
        used_percent,
    })
}

fn percent(value: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (value as f64 / total as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::session::JumpHostPlan;

    fn ssh_plan(host: &str, username: &str, profile_name: &str) -> SessionPlan {
        SessionPlan {
            kind: SessionKind::Ssh,
            profile_id: None,
            profile_name: profile_name.to_string(),
            host: Some(host.to_string()),
            port: 22,
            username: Some(username.to_string()),
            password: None,
            ignore_saved_password: false,
            remember_password: false,
            keepalive_interval_secs: 15,
            keepalive_count_max: 3,
            private_key_path: None,
            private_key_passphrase: None,
            terminal_shell: None,
            jump_hosts: Vec::new(),
        }
    }

    #[test]
    fn monitor_connection_fingerprint_distinguishes_embedded_separators() {
        let first = ssh_plan("b", "c", "a|");
        let second = ssh_plan("|b", "c", "a");

        assert_ne!(
            monitor_connection_fingerprint(&first).unwrap(),
            monitor_connection_fingerprint(&second).unwrap()
        );
    }

    #[test]
    fn monitor_connection_fingerprint_distinguishes_jump_host_boundaries() {
        let mut first = ssh_plan("target", "user", "profile");
        first.jump_hosts.push(JumpHostPlan {
            host: "a|b".to_string(),
            port: 22,
            username: "jump".to_string(),
            password: None,
            private_key_path: None,
            private_key_passphrase: None,
        });

        let mut second = ssh_plan("target", "user", "profile");
        second.jump_hosts.push(JumpHostPlan {
            host: "a".to_string(),
            port: 22,
            username: "b|jump".to_string(),
            password: None,
            private_key_path: None,
            private_key_passphrase: None,
        });

        assert_ne!(
            monitor_connection_fingerprint(&first).unwrap(),
            monitor_connection_fingerprint(&second).unwrap()
        );
    }
}
