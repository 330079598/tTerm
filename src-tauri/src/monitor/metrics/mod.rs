//! Collects a host's metrics with one shell script and turns its output into
//! a [`ServerMetricsSnapshot`]. Linux, FreeBSD and macOS each have their own
//! sources, so each has its own section of the script and its own parser;
//! what they report the same way is read here.

mod bsd;
mod darwin;
mod freebsd;
mod linux;
#[cfg(all(test, unix))]
mod real_sshd_tests;
#[cfg(test)]
mod tests;

use super::types::{DiskMetrics, LinuxDistributionInfo, LoadAverageMetrics, ServerMetricsSnapshot};

/// Prints the metrics between `__TTERM_*__` markers. Run it under `sh -s`
/// with the script on standard input, which works whatever the account's
/// login shell is.
pub(crate) const METRICS_SCRIPT: &str = include_str!("collect.sh");

pub(crate) fn parse_metrics_output(output: &str) -> ServerMetricsSnapshot {
    let uname = find_prefixed_line(output, "__TTERM_UNAME__").unwrap_or_default();
    let mut snapshot = match uname.split_whitespace().next() {
        Some("Linux") => linux::parse(output),
        Some("FreeBSD") => freebsd::parse(output, &uname),
        Some("Darwin") => darwin::parse(output),
        _ => ServerMetricsSnapshot::default(),
    };
    if snapshot.distribution.is_none() {
        let os_release = extract_block(
            output,
            "__TTERM_OS_RELEASE_BEGIN__",
            "__TTERM_OS_RELEASE_END__",
        );
        snapshot.distribution = parse_os_release(os_release.unwrap_or_default());
    }
    snapshot.kernel = (!uname.is_empty()).then_some(uname);
    snapshot.primary_ip =
        find_prefixed_line(output, "__TTERM_PRIMARY_IP__").and_then(|line| parse_primary_ip(&line));
    snapshot
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

/// Reads three load averages, with or without the braces BSD's
/// `vm.loadavg` puts around them.
pub(super) fn parse_load_average(line: &str) -> Option<LoadAverageMetrics> {
    let mut values = line
        .split_whitespace()
        .filter(|value| *value != "{" && *value != "}");
    Some(LoadAverageMetrics {
        one: values.next()?.parse().ok()?,
        five: values.next()?.parse().ok()?,
        fifteen: values.next()?.parse().ok()?,
    })
}

/// Puts the root file system first and the rest by mount point.
pub(super) fn sort_disks(disks: &mut [DiskMetrics]) {
    disks.sort_by(|left, right| {
        (left.mount != "/")
            .cmp(&(right.mount != "/"))
            .then_with(|| left.mount.cmp(&right.mount))
    });
}

pub(crate) fn percent(value: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (value as f64 / total as f64) * 100.0
    }
}
