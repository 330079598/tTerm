//! Reads what the script's macOS section prints: the version from
//! `sw_vers`, memory from `vm_stat` and `sysctl`, and the CPU from `iostat`.

use super::bsd::{parse_common, Sysctl};
use super::extract_block;
use crate::monitor::types::{LinuxDistributionInfo, MemoryMetrics, ServerMetricsSnapshot};
use std::collections::HashMap;

pub(super) fn parse(output: &str) -> ServerMetricsSnapshot {
    let sysctl = Sysctl::parse(output);
    let block = |name: &str| {
        extract_block(
            output,
            &format!("__TTERM_{name}_BEGIN__"),
            &format!("__TTERM_{name}_END__"),
        )
        .unwrap_or_default()
    };

    ServerMetricsSnapshot {
        distribution: parse_sw_vers(block("SW_VERS")),
        cpu_percent: parse_iostat(block("IOSTAT")),
        memory: parse_memory(&sysctl, block("VM_STAT")),
        ..parse_common(output, &sysctl)
    }
}

/// `sw_vers`'s `ProductName: macOS` lines, as an os-release would say it.
fn parse_sw_vers(block: &str) -> Option<LinuxDistributionInfo> {
    let values = block
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim(), value.trim()))
        .collect::<HashMap<_, _>>();
    let name = values.get("ProductName").copied().unwrap_or("macOS");
    let version = values.get("ProductVersion").copied()?;
    Some(LinuxDistributionInfo {
        id: "macos".to_string(),
        id_like: Vec::new(),
        name: name.to_string(),
        pretty_name: format!("{name} {version}"),
        version_id: Some(version.to_string()),
    })
}

/// The CPU use over the last interval `iostat` printed. Its first row is the
/// average since boot, so a sample needs the second.
fn parse_iostat(block: &str) -> Option<f64> {
    let mut lines = block
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>());
    let idle_column = lines
        .by_ref()
        .find_map(|words| words.iter().position(|word| *word == "id"))?;
    let idle = lines.nth(1)?.get(idle_column)?.parse::<f64>().ok()?;
    Some((100.0 - idle).clamp(0.0, 100.0))
}

/// Memory as Activity Monitor counts it: used is app memory (anonymous
/// pages the apps have not marked purgeable), wired memory and what the
/// compressor occupies. File-backed and purgeable pages can be handed out
/// at once, so they count as available, as the page cache does on Linux.
fn parse_memory(sysctl: &Sysctl, vm_stat: &str) -> Option<MemoryMetrics> {
    let total = sysctl.number("hw.memsize")?;
    let mut lines = vm_stat.lines();
    // "Mach Virtual Memory Statistics: (page size of 16384 bytes)"
    let page_size = lines
        .next()?
        .split("page size of ")
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?;
    let pages = lines
        .filter_map(|line| {
            let (name, value) = line.rsplit_once(':')?;
            let count = value.trim().trim_end_matches('.').parse::<u64>().ok()?;
            Some((name.trim(), count))
        })
        .collect::<HashMap<_, _>>();
    let count = |name: &str| pages.get(name).copied().unwrap_or(0);
    // Before OS X 10.9 there were no anonymous pages; active ones come close.
    let app = pages
        .get("Anonymous pages")
        .copied()
        .unwrap_or_else(|| count("Pages active"))
        .saturating_sub(count("Pages purgeable"));
    let used =
        (app + count("Pages wired down") + count("Pages occupied by compressor")) * page_size;
    let (swap_total, swap_free) = sysctl
        .get("vm.swapusage")
        .and_then(parse_swap_usage)
        .unwrap_or_default();

    Some(MemoryMetrics::new(
        total / 1024,
        total.saturating_sub(used) / 1024,
        swap_total,
        swap_free,
    ))
}

/// The total and free swap in KiB, from
/// `total = 2048.00M  used = 1069.75M  free = 978.25M  (encrypted)`.
fn parse_swap_usage(value: &str) -> Option<(u64, u64)> {
    let amount = |name: &str| -> Option<u64> {
        let mut words = value.split_whitespace();
        words.find(|word| *word == name)?;
        words.next().filter(|word| *word == "=")?;
        let size = words.next()?;
        let (number, kib_per_unit) = [('K', 1.0), ('M', 1024.0), ('G', 1024.0 * 1024.0)]
            .into_iter()
            .find_map(|(unit, kib)| Some((size.strip_suffix(unit)?, kib)))?;
        Some((number.parse::<f64>().ok()? * kib_per_unit).round() as u64)
    };
    Some((amount("total")?, amount("free")?))
}
