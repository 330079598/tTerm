//! Reads what the script's Linux section prints: `/proc` for the CPU,
//! memory, load, uptime and network, and `df` for the file systems.

use super::{extract_block, find_prefixed_line, parse_load_average, percent, sort_disks};
use crate::monitor::types::{
    CpuCoreTimes, CpuTimes, DiskMetrics, MemoryMetrics, NetworkMetrics, ServerMetricsSnapshot,
};

pub(super) fn parse(output: &str) -> ServerMetricsSnapshot {
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
        supported: true,
        cpu_times: find_prefixed_line(output, "__TTERM_PROC_STAT__")
            .and_then(|line| parse_cpu_times(&line)),
        cpu_core_times: find_prefixed_line(output, "__TTERM_PROC_STAT_CORES__")
            .map(|line| parse_cpu_core_times(&line))
            .unwrap_or_default(),
        memory: find_prefixed_line(output, "__TTERM_MEMINFO__")
            .and_then(|line| parse_memory_metrics(&line)),
        disk,
        disks,
        network: find_prefixed_line(output, "__TTERM_NETWORK__")
            .and_then(|line| parse_network_metrics(&line)),
        load_average: find_prefixed_line(output, "__TTERM_LOADAVG__")
            .and_then(|line| parse_load_average(&line)),
        uptime_secs: find_prefixed_line(output, "__TTERM_UPTIME__")
            .and_then(|line| line.parse::<u64>().ok()),
        ..ServerMetricsSnapshot::default()
    }
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
    Some(MemoryMetrics::new(
        total,
        available,
        swap_total.unwrap_or(0),
        swap_free.unwrap_or(0),
    ))
}

fn parse_legacy_disk_metrics(line: &str) -> Option<DiskMetrics> {
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
    sort_disks(&mut disks);
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
