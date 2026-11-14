//! Reads what FreeBSD and macOS report the same way: `sysctl`'s
//! `name: value` lines, the default interface's `netstat -ibn` counters,
//! and the file systems from `df -P` and `mount`.

use super::{extract_block, find_prefixed_line, parse_load_average, percent, sort_disks};
use crate::monitor::types::{DiskMetrics, NetworkMetrics, ServerMetricsSnapshot};
use std::collections::HashMap;

/// File system types that hold no user data: kernel interfaces, memory
/// disks, automounter maps and loopback mounts of another file system.
const PSEUDO_FILESYSTEMS: &[&str] = &[
    "autofs",
    "devfs",
    "fdescfs",
    "linprocfs",
    "linsysfs",
    "mfs",
    "nullfs",
    "procfs",
    "tmpfs",
];

/// The values `sysctl` printed, by name.
pub(super) struct Sysctl<'a>(HashMap<&'a str, &'a str>);

impl<'a> Sysctl<'a> {
    pub(super) fn parse(output: &'a str) -> Self {
        let block = extract_block(output, "__TTERM_SYSCTL_BEGIN__", "__TTERM_SYSCTL_END__");
        Self(
            block
                .unwrap_or_default()
                .lines()
                .filter_map(|line| line.split_once(": "))
                .map(|(name, value)| (name.trim(), value.trim()))
                .collect(),
        )
    }

    pub(super) fn get(&self, name: &str) -> Option<&'a str> {
        self.0.get(name).copied()
    }

    pub(super) fn number(&self, name: &str) -> Option<u64> {
        self.get(name)?.parse().ok()
    }
}

/// The parts of a sample both systems report alike, marked supported.
pub(super) fn parse_common(output: &str, sysctl: &Sysctl) -> ServerMetricsSnapshot {
    let disks = parse_disks(
        extract_block(output, "__TTERM_DF_BEGIN__", "__TTERM_DF_END__").unwrap_or_default(),
        extract_block(output, "__TTERM_MOUNT_BEGIN__", "__TTERM_MOUNT_END__").unwrap_or_default(),
    );
    let now = find_prefixed_line(output, "__TTERM_NOW__").and_then(|line| line.parse().ok());

    ServerMetricsSnapshot {
        supported: true,
        disk: disks.iter().find(|disk| disk.mount == "/").cloned(),
        disks,
        network: extract_block(output, "__TTERM_NETSTAT_BEGIN__", "__TTERM_NETSTAT_END__")
            .and_then(parse_netstat),
        load_average: sysctl.get("vm.loadavg").and_then(parse_load_average),
        uptime_secs: now
            .zip(sysctl.get("kern.boottime").and_then(parse_boot_time))
            .map(|(now, boot): (u64, u64)| now.saturating_sub(boot)),
        ..ServerMetricsSnapshot::default()
    }
}

/// The seconds part of `kern.boottime`, which prints as
/// `{ sec = 1700000000, usec = 123456 } Tue Nov 14 22:13:20 2023`.
fn parse_boot_time(value: &str) -> Option<u64> {
    let mut words = value.split_whitespace();
    words.find(|word| *word == "sec")?;
    words.next().filter(|word| *word == "=")?;
    words.next()?.trim_end_matches(',').parse().ok()
}

/// The counters on the interface's link-level row. The columns differ
/// between systems (FreeBSD has `Idrop`, macOS does not), so they are
/// found by the header's names. Counting from the right, since the address
/// column before the counters is empty on an interface without one.
pub(super) fn parse_netstat(block: &str) -> Option<NetworkMetrics> {
    let mut lines = block.lines();
    let header = lines.next()?.split_whitespace().collect::<Vec<_>>();
    let fields = lines
        .find(|line| line.contains("<Link#"))?
        .split_whitespace()
        .collect::<Vec<_>>();
    let column = |name: &str| -> Option<u64> {
        let from_end = header.len() - header.iter().position(|column| *column == name)?;
        fields
            .get(fields.len().checked_sub(from_end)?)?
            .parse()
            .ok()
    };

    Some(NetworkMetrics {
        interface: fields.first()?.trim_end_matches('*').to_string(),
        received_bytes: column("Ibytes")?,
        transmitted_bytes: column("Obytes")?,
        receive_errors: column("Ierrs").unwrap_or(0),
        transmit_errors: column("Oerrs").unwrap_or(0),
        receive_dropped: column("Idrop").unwrap_or(0),
        transmit_dropped: column("Odrop").or_else(|| column("Drop")).unwrap_or(0),
    })
}

/// A `mount` line's file system type and options, from
/// `/dev/ada0p2 on / (ufs, local, soft-updates)`.
struct MountInfo<'a> {
    filesystem_type: &'a str,
    options: Vec<&'a str>,
}

fn parse_mounts(block: &str) -> HashMap<&str, MountInfo<'_>> {
    block
        .lines()
        .filter_map(|line| {
            let (device_and_mount, options) =
                line.trim_end().strip_suffix(')')?.rsplit_once(" (")?;
            let (_, mount) = device_and_mount.split_once(" on ")?;
            let mut options = options.split(',').map(str::trim);
            let filesystem_type = options.next()?;
            Some((
                mount,
                MountInfo {
                    filesystem_type,
                    options: options.collect(),
                },
            ))
        })
        .collect()
}

/// The file systems worth showing, from `df -P -k` and `mount`.
pub(super) fn parse_disks(df: &str, mount: &str) -> Vec<DiskMetrics> {
    let mounts = parse_mounts(mount);
    let disks = df
        .lines()
        .skip(1)
        .filter_map(parse_df_line)
        .filter_map(|mut disk| {
            let info = mounts.get(disk.mount.as_str());
            if let Some(info) = info {
                // macOS hides its system volumes from the Finder this way.
                if PSEUDO_FILESYSTEMS.contains(&info.filesystem_type)
                    || info.options.contains(&"nobrowse")
                {
                    return None;
                }
                disk.filesystem_type = info.filesystem_type.to_string();
            }
            if disk.total_kib == 0 {
                return None;
            }
            if disk.filesystem_type == "apfs" {
                // An APFS volume shares its container's space with the
                // other volumes in it, and `df` counts only its own data as
                // used: the sealed system volume at `/` reads a few percent
                // full on a nearly full disk. What the container has left
                // is what matters, as in the Finder.
                disk.used_kib = disk.total_kib.saturating_sub(disk.available_kib);
                disk.used_percent = percent(disk.used_kib, disk.total_kib);
            }
            Some(disk)
        })
        .collect::<Vec<_>>();
    let mut disks = fold_zfs_pools(disks);
    sort_disks(&mut disks);
    disks
}

/// One `df -P` row. The capacity column, the first one ending in `%` after
/// three numbers, splits it: a device such as macOS's `map auto_home` and a
/// mount point may both contain spaces.
fn parse_df_line(line: &str) -> Option<DiskMetrics> {
    let words = line.split_whitespace().collect::<Vec<_>>();
    let capacity = (4..words.len()).find(|&index| {
        words[index].ends_with('%')
            && words[index - 3..index]
                .iter()
                .all(|word| word.parse::<u64>().is_ok())
    })?;
    let total = words[capacity - 3].parse::<u64>().ok()?;
    let used = words[capacity - 2].parse::<u64>().ok()?;
    let available = words[capacity - 1].parse::<u64>().ok()?;
    let mount = words[capacity + 1..].join(" ");
    if mount.is_empty() {
        return None;
    }

    Some(DiskMetrics {
        filesystem: words[..capacity - 3].join(" "),
        filesystem_type: String::new(),
        mount,
        total_kib: total,
        used_kib: used,
        available_kib: available,
        used_percent: words[capacity]
            .trim_end_matches('%')
            .parse()
            .unwrap_or_else(|_| percent(used, total)),
    })
}

/// ZFS datasets share their pool's free space, and `df` shows each one's
/// own data against it, so a default FreeBSD install would list a dozen
/// nearly empty file systems. They become one row per pool: the data of all
/// its mounted datasets against what the pool has left, under the dataset
/// mounted at `/` or else the shortest mount point.
fn fold_zfs_pools(disks: Vec<DiskMetrics>) -> Vec<DiskMetrics> {
    let mut pools: Vec<DiskMetrics> = Vec::new();
    let mut others = Vec::new();
    for disk in disks {
        if disk.filesystem_type != "zfs" {
            others.push(disk);
            continue;
        }
        let pool = disk.filesystem.split('/').next().unwrap_or_default();
        let Some(row) = pools.iter_mut().find(|row| row.filesystem == pool) else {
            pools.push(DiskMetrics {
                filesystem: pool.to_string(),
                ..disk
            });
            continue;
        };
        row.used_kib += disk.used_kib;
        row.available_kib = row.available_kib.max(disk.available_kib);
        let better_mount =
            disk.mount == "/" || (row.mount != "/" && disk.mount.len() < row.mount.len());
        if better_mount {
            row.mount = disk.mount;
        }
    }
    for pool in &mut pools {
        pool.total_kib = pool.used_kib + pool.available_kib;
        pool.used_percent = percent(pool.used_kib, pool.total_kib);
    }
    others.extend(pools);
    others
}
