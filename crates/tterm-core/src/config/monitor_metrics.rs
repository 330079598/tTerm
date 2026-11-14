//! The figures the server monitor bar can show, from
//! `src/lib/monitorMetrics.ts`.

/// A figure the server monitor bar can show for an SSH host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorMetricId {
    /// Processor use.
    Cpu,
    /// Memory use.
    Memory,
    /// Network throughput.
    Network,
    /// The host's addresses.
    Ip,
    /// Round-trip time.
    Latency,
    /// Disk use.
    Disk,
    /// Load averages.
    Load,
    /// Time since boot.
    Uptime,
}

/// The metrics shown when the user saved none.
pub const DEFAULT_MONITOR_VISIBLE_METRICS: [MonitorMetricId; 6] = [
    MonitorMetricId::Cpu,
    MonitorMetricId::Memory,
    MonitorMetricId::Network,
    MonitorMetricId::Ip,
    MonitorMetricId::Latency,
    MonitorMetricId::Disk,
];

impl MonitorMetricId {
    /// The id the setting stores.
    pub fn as_str(self) -> &'static str {
        match self {
            MonitorMetricId::Cpu => "cpu",
            MonitorMetricId::Memory => "memory",
            MonitorMetricId::Network => "network",
            MonitorMetricId::Ip => "ip",
            MonitorMetricId::Latency => "latency",
            MonitorMetricId::Disk => "disk",
            MonitorMetricId::Load => "load",
            MonitorMetricId::Uptime => "uptime",
        }
    }

    /// The metric a stored id names, or `None` for anything unknown.
    pub fn parse(value: &str) -> Option<MonitorMetricId> {
        [
            MonitorMetricId::Cpu,
            MonitorMetricId::Memory,
            MonitorMetricId::Network,
            MonitorMetricId::Ip,
            MonitorMetricId::Latency,
            MonitorMetricId::Disk,
            MonitorMetricId::Load,
            MonitorMetricId::Uptime,
        ]
        .into_iter()
        .find(|metric| metric.as_str() == value)
    }
}

/// The saved metrics in their order, without unknown ids or repeats; the
/// defaults when none is left. `None` stands for a value that is not a list.
pub fn normalize_monitor_visible_metrics(value: Option<&[String]>) -> Vec<MonitorMetricId> {
    let Some(value) = value else {
        return DEFAULT_MONITOR_VISIBLE_METRICS.to_vec();
    };

    let mut metrics: Vec<MonitorMetricId> = Vec::new();
    for item in value {
        if let Some(metric) = MonitorMetricId::parse(item)
            && !metrics.contains(&metric)
        {
            metrics.push(metric);
        }
    }
    if metrics.is_empty() {
        return DEFAULT_MONITOR_VISIBLE_METRICS.to_vec();
    }
    metrics
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_MONITOR_VISIBLE_METRICS, MonitorMetricId, normalize_monitor_visible_metrics,
    };

    fn stored(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn keeps_known_metrics_in_their_order_once_each() {
        assert_eq!(
            normalize_monitor_visible_metrics(Some(&stored(&["uptime", "cpu", "bogus", "cpu"]))),
            vec![MonitorMetricId::Uptime, MonitorMetricId::Cpu]
        );
    }

    #[test]
    fn falls_back_to_the_defaults() {
        assert_eq!(
            normalize_monitor_visible_metrics(None),
            DEFAULT_MONITOR_VISIBLE_METRICS
        );
        assert_eq!(
            normalize_monitor_visible_metrics(Some(&stored(&["bogus"]))),
            DEFAULT_MONITOR_VISIBLE_METRICS
        );
    }
}
