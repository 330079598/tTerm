//! Reads what the script's FreeBSD section prints: CPU ticks, memory pages
//! and the ZFS cache from `sysctl`, swap from `swapinfo`.

use super::bsd::{parse_common, Sysctl};
use super::{extract_block, parse_os_release};
use crate::monitor::types::{
    CpuCoreTimes, CpuTimes, LinuxDistributionInfo, MemoryMetrics, ServerMetricsSnapshot,
};

/// `kern.cp_time` counts user, nice, system, interrupt and idle ticks.
const CPU_STATES: usize = 5;

pub(super) fn parse(output: &str, uname: &str) -> ServerMetricsSnapshot {
    let sysctl = Sysctl::parse(output);
    let os_release = extract_block(
        output,
        "__TTERM_OS_RELEASE_BEGIN__",
        "__TTERM_OS_RELEASE_END__",
    );
    let swapinfo = extract_block(output, "__TTERM_SWAPINFO_BEGIN__", "__TTERM_SWAPINFO_END__");

    ServerMetricsSnapshot {
        // FreeBSD has had `/etc/os-release` since 13.0.
        distribution: parse_os_release(os_release.unwrap_or_default())
            .or_else(|| distribution_from_uname(uname)),
        cpu_times: sysctl
            .get("kern.cp_time")
            .and_then(|value| parse_ticks(value).first().cloned()),
        cpu_core_times: sysctl
            .get("kern.cp_times")
            .map(|value| {
                parse_ticks(value)
                    .into_iter()
                    .enumerate()
                    .map(|(index, times)| CpuCoreTimes {
                        id: format!("cpu{index}"),
                        times,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        memory: parse_memory(&sysctl, swapinfo.unwrap_or_default()),
        ..parse_common(output, &sysctl)
    }
}

/// One `CpuTimes` per group of five tick counters: `kern.cp_time` has one
/// group, `kern.cp_times` one per core.
fn parse_ticks(value: &str) -> Vec<CpuTimes> {
    let Ok(ticks) = value
        .split_whitespace()
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
    else {
        return Vec::new();
    };
    let (groups, _) = ticks.as_chunks::<CPU_STATES>();
    groups
        .iter()
        .map(|&[user, nice, system, irq, idle]| CpuTimes {
            user,
            nice,
            system,
            irq,
            idle,
            ..CpuTimes::default()
        })
        .collect()
}

/// Memory the way `top` splits it: free, inactive and (before FreeBSD 12)
/// cached pages can be handed out at once. The ZFS cache sits in wired
/// memory but gives way down to its minimum size, so that much of it counts
/// as available too, as the page cache does on Linux.
fn parse_memory(sysctl: &Sysctl, swapinfo: &str) -> Option<MemoryMetrics> {
    let page_size = sysctl.number("hw.pagesize")?;
    let pages = |name: &str| sysctl.number(name).unwrap_or(0) * page_size;
    let total = sysctl
        .number("vm.stats.vm.v_page_count")
        .map(|count| count * page_size)
        .or_else(|| sysctl.number("hw.physmem"))?;
    let zfs_cache = sysctl
        .number("kstat.zfs.misc.arcstats.size")
        .unwrap_or(0)
        .saturating_sub(sysctl.number("kstat.zfs.misc.arcstats.c_min").unwrap_or(0));
    let available = pages("vm.stats.vm.v_free_count")
        + pages("vm.stats.vm.v_inactive_count")
        + pages("vm.stats.vm.v_cache_count")
        + zfs_cache;
    let (swap_total, swap_free) = parse_swapinfo(swapinfo);

    Some(MemoryMetrics::new(
        total / 1024,
        available / 1024,
        swap_total,
        swap_free,
    ))
}

/// The total and free swap in KiB, summed over the devices `swapinfo -k`
/// lists (it adds a `Total` row of its own when there are several).
fn parse_swapinfo(block: &str) -> (u64, u64) {
    block
        .lines()
        .skip(1)
        .filter(|line| !line.starts_with("Total"))
        .filter_map(|line| {
            let mut words = line.split_whitespace().skip(1);
            let total = words.next()?.parse::<u64>().ok()?;
            let free = words.nth(1)?.parse::<u64>().ok()?;
            Some((total, free))
        })
        .fold((0, 0), |(total, free), (device_total, device_free)| {
            (total + device_total, free + device_free)
        })
}

/// For a FreeBSD older than 13.0: `FreeBSD 12.4-RELEASE-p9 amd64`.
fn distribution_from_uname(uname: &str) -> Option<LinuxDistributionInfo> {
    let release = uname.split_whitespace().nth(1)?;
    Some(LinuxDistributionInfo {
        id: "freebsd".to_string(),
        id_like: Vec::new(),
        name: "FreeBSD".to_string(),
        pretty_name: format!("FreeBSD {release}"),
        version_id: release.split('-').next().map(str::to_string),
    })
}
