//! Reads what the script prints on Linux: `/proc` for the CPU, memory, load,
//! uptime and network, and `df` for the file systems.

use super::percent;
use crate::monitor::types::{CpuCoreTimes, CpuTimes, DiskMetrics, MemoryMetrics, NetworkMetrics};

pub(super) fn parse_cpu_times(line: &str) -> Option<CpuTimes> {
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

pub(super) fn parse_cpu_core_times(line: &str) -> Vec<CpuCoreTimes> {
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

pub(super) fn parse_memory_metrics(line: &str) -> Option<MemoryMetrics> {
    let mut total = None;
    let mut available = None;
    let mut free = None;
    let mut buffers = None;
    let mut cached = None;
    let mut reclaimable = None;
    let mut swap_total = None;
    let mut swap_free = None;

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
            "SwapTotal" => swap_total = value,
            "SwapFree" => swap_free = value,
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
    let swap_total = swap_total.unwrap_or(0);
    let swap_free = swap_free.unwrap_or(0).min(swap_total);
    let swap_used = swap_total.saturating_sub(swap_free);

    Some(MemoryMetrics {
        total_kib: total,
        available_kib: available,
        used_kib: used,
        used_percent,
        swap_total_kib: swap_total,
        swap_free_kib: swap_free,
        swap_used_kib: swap_used,
        swap_used_percent: percent(swap_used, swap_total),
    })
}

pub(super) fn parse_legacy_disk_metrics(line: &str) -> Option<DiskMetrics> {
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
        filesystem: String::new(),
        filesystem_type: String::new(),
        mount,
        total_kib: total,
        used_kib: used,
        available_kib: available,
        used_percent,
    })
}

pub(super) fn parse_disk_metrics_block(block: &str) -> Vec<DiskMetrics> {
    let mut disks = block
        .lines()
        .filter_map(|line| {
            let mut values = line.split('\t');
            let filesystem = values.next()?.to_string();
            let filesystem_type = values.next()?.to_string();
            let total = values.next()?.parse::<u64>().ok()?;
            let used = values.next()?.parse::<u64>().ok()?;
            let available = values.next()?.parse::<u64>().ok()?;
            let raw_percent = values.next()?.trim_end_matches('%');
            let mount = values.next()?.to_string();
            let used_percent = raw_percent
                .parse::<f64>()
                .unwrap_or_else(|_| percent(used, total));

            Some(DiskMetrics {
                filesystem,
                filesystem_type,
                mount,
                total_kib: total,
                used_kib: used,
                available_kib: available,
                used_percent,
            })
        })
        .collect::<Vec<_>>();
    disks.sort_by(|left, right| {
        (left.mount != "/")
            .cmp(&(right.mount != "/"))
            .then_with(|| left.mount.cmp(&right.mount))
    });
    disks
}

pub(super) fn parse_network_metrics(line: &str) -> Option<NetworkMetrics> {
    let mut values = line.split_whitespace();
    Some(NetworkMetrics {
        interface: values.next()?.to_string(),
        received_bytes: values.next()?.parse().ok()?,
        transmitted_bytes: values.next()?.parse().ok()?,
        receive_errors: values.next()?.parse().ok()?,
        transmit_errors: values.next()?.parse().ok()?,
        receive_dropped: values.next()?.parse().ok()?,
        transmit_dropped: values.next()?.parse().ok()?,
    })
}
