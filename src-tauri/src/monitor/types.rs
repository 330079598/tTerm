use super::metrics::percent;
use crate::ssh::jump::connect::JumpChain;
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
    pub ssh: client::Handle<crate::ssh::types::SshClientHandler>,
}

/// What a host says it runs, in `/etc/os-release` terms. Linux and FreeBSD
/// have that file; the monitor fills it in from `sw_vers` on macOS.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinuxDistributionInfo {
    pub id: String,
    pub id_like: Vec<String>,
    pub name: String,
    pub pretty_name: String,
    pub version_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
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

impl MemoryMetrics {
    /// Works out the used amounts and percentages from the totals, in KiB.
    pub fn new(total: u64, available: u64, swap_total: u64, swap_free: u64) -> Self {
        let available = available.min(total);
        let used = total - available;
        let swap_free = swap_free.min(swap_total);
        let swap_used = swap_total - swap_free;
        Self {
            total_kib: total,
            available_kib: available,
            used_kib: used,
            used_percent: percent(used, total),
            swap_total_kib: swap_total,
            swap_free_kib: swap_free,
            swap_used_kib: swap_used,
            swap_used_percent: percent(swap_used, swap_total),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskMetrics {
    pub filesystem: String,
    pub filesystem_type: String,
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

/// One sample of a host's metrics. A host whose system the monitor cannot
/// read is `supported: false`, with only `kernel` (`uname -srm`) set.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerMetricsSnapshot {
    pub supported: bool,
    pub distribution: Option<LinuxDistributionInfo>,
    pub kernel: Option<String>,
    /// Counters to diff against the previous sample, where the host keeps them.
    pub cpu_times: Option<CpuTimes>,
    /// The CPU use the host measured itself, on macOS, which keeps no
    /// counters a shell can read.
    pub cpu_percent: Option<f64>,
    pub cpu_core_times: Vec<CpuCoreTimes>,
    pub memory: Option<MemoryMetrics>,
    pub primary_ip: Option<String>,
    pub disk: Option<DiskMetrics>,
    pub disks: Vec<DiskMetrics>,
    pub network: Option<NetworkMetrics>,
    pub load_average: Option<LoadAverageMetrics>,
    pub uptime_secs: Option<u64>,
    pub network_latency_ms: Option<u64>,
}
