use crate::ssh::JumpChain;
use russh::client;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Instant;

pub(crate) const MONITOR_SESSION_IDLE_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(5 * 60);

pub type MonitorSessionMap =
    Arc<Mutex<std::collections::HashMap<String, Arc<Mutex<MonitorSession>>>>>;

pub struct MonitorSession {
    pub session_nonce: u32,
    pub fingerprint: String,
    pub last_used_at: Instant,
    pub jump_chain: Option<JumpChain>,
    pub ssh: client::Handle<crate::ssh::SshClientHandler>,
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
    pub swap_total_kib: u64,
    pub swap_free_kib: u64,
    pub swap_used_kib: u64,
    pub swap_used_percent: f64,
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
pub struct NetworkMetrics {
    pub interface: String,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
    pub receive_errors: u64,
    pub transmit_errors: u64,
    pub receive_dropped: u64,
    pub transmit_dropped: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadAverageMetrics {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
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
    pub network: Option<NetworkMetrics>,
    pub load_average: Option<LoadAverageMetrics>,
    pub uptime_secs: Option<u64>,
    pub network_latency_ms: Option<u64>,
}
