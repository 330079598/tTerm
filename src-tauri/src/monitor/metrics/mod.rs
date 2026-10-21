//! Collects a host's metrics with one shell script and turns its output into
//! a [`ServerMetricsSnapshot`].

mod linux;
#[cfg(test)]
mod tests;

use super::types::{LinuxDistributionInfo, LoadAverageMetrics, ServerMetricsSnapshot};
use linux::{
    parse_cpu_core_times, parse_cpu_times, parse_disk_metrics_block, parse_legacy_disk_metrics,
    parse_memory_metrics, parse_network_metrics,
};

/// Prints the metrics between `__TTERM_*__` markers.
pub(crate) const METRICS_SCRIPT: &str = include_str!("collect.sh");

pub(crate) fn parse_metrics_output(output: &str) -> ServerMetricsSnapshot {
    let os_release = extract_block(
        output,
        "__TTERM_OS_RELEASE_BEGIN__",
        "__TTERM_OS_RELEASE_END__",
    );
    let uname = find_prefixed_line(output, "__TTERM_UNAME__").unwrap_or_default();
    let kernel = (!uname.trim().is_empty()).then(|| uname.trim().to_string());
    let supported = uname.split_whitespace().next() == Some("Linux");

    let disks = extract_block(output, "__TTERM_DISKS_BEGIN__", "__TTERM_DISKS_END__")
        .map(parse_disk_metrics_block)
        .unwrap_or_default();
    let disk = disks
        .iter()
        .find(|disk| disk.mount == "/")
        .cloned()
        .or_else(|| {
            find_prefixed_line(output, "__TTERM_DF_ROOT__")
                .and_then(|line| parse_legacy_disk_metrics(&line))
        });

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
        disk,
        disks,
        network: find_prefixed_line(output, "__TTERM_NETWORK__")
            .and_then(|line| parse_network_metrics(&line)),
        load_average: find_prefixed_line(output, "__TTERM_LOADAVG__")
            .and_then(|line| parse_load_average(&line)),
        uptime_secs: find_prefixed_line(output, "__TTERM_UPTIME__")
            .and_then(|line| line.parse::<u64>().ok()),
        network_latency_ms: None,
    }
}

pub(super) fn extract_block<'a>(output: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_idx = output.find(start)? + start.len();
    let rest = &output[start_idx..];
    let end_idx = rest.find(end)?;
    Some(rest[..end_idx].trim())
}

pub(super) fn find_prefixed_line(output: &str, prefix: &str) -> Option<String> {
    output.lines().find_map(|line| {
        line.strip_prefix(prefix)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

pub(crate) fn parse_os_release(content: &str) -> Option<LinuxDistributionInfo> {
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

fn parse_primary_ip(line: &str) -> Option<String> {
    line.split_whitespace()
        .find(|value| !value.starts_with("127.") && !value.starts_with("169.254."))
        .map(str::to_string)
}

fn parse_load_average(line: &str) -> Option<LoadAverageMetrics> {
    let mut values = line.split_whitespace();
    Some(LoadAverageMetrics {
        one: values.next()?.parse().ok()?,
        five: values.next()?.parse().ok()?,
        fifteen: values.next()?.parse().ok()?,
    })
}

pub(crate) fn percent(value: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (value as f64 / total as f64) * 100.0
    }
}
