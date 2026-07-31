use super::types::{
    CpuCoreTimes, CpuTimes, DiskMetrics, LinuxDistributionInfo, LoadAverageMetrics, MemoryMetrics,
    NetworkMetrics, ServerMetricsSnapshot,
};

pub(crate) const METRICS_SCRIPT: &str = r#"    set -efu

    echo "__TTERM_OS_RELEASE_BEGIN__"
    if [ -r /etc/os-release ]; then
      cat /etc/os-release
    fi
    echo "__TTERM_OS_RELEASE_END__"

    printf "__TTERM_UNAME__ "
    uname -srm 2>/dev/null || true

    if [ "$(uname -s 2>/dev/null || true)" != "Linux" ]; then
      exit 0
    fi

    printf "__TTERM_PROC_STAT__ "
    awk '
      /^cpu / {
        print $2, $3, $4, $5, $6, $7, $8, $9, $10, $11
      }
    ' /proc/stat 2>/dev/null || true

    printf "__TTERM_PROC_STAT_CORES__ "
    awk '
      /^cpu[0-9]+ / {
        times = $2 "," $3 "," $4 "," $5 "," $6 "," $7 "," $8 "," $9 "," $10 "," $11
        printf "%s=%s ", $1, times
      }
      END {
        printf "\n"
      }
    ' /proc/stat 2>/dev/null || true

    printf "__TTERM_MEMINFO__ "
    awk '
      /^(MemTotal|MemAvailable|MemFree|Buffers|Cached|SReclaimable|SwapTotal|SwapFree):/ {
        gsub(":", "", $1)
        printf "%s=%s ", $1, $2
      }
      END {
        printf "\n"
      }
    ' /proc/meminfo 2>/dev/null || true

    printf "__TTERM_PRIMARY_IP__ "
    (
      ip -o -4 route get 1.1.1.1 2>/dev/null | awk '
        {
          for (i = 1; i <= NF; i++) {
            if ($i == "src") {
              print $(i + 1)
              exit
            }
          }
        }
      '

      hostname -I 2>/dev/null | awk '
        {
          for (i = 1; i <= NF; i++) {
            if ($i !~ /^127\./ && $i !~ /^169\.254\./) {
              print $i
              exit
            }
          }
        }
      '

      ip -o -4 addr show scope global 2>/dev/null | awk '
        {
          split($4, parts, "/")
          print parts[1]
          exit
        }
      '
    ) | awk 'NF { print; exit }'
    printf "\n"

    printf "__TTERM_DF_ROOT__ "
    df -P -k / 2>/dev/null | awk 'NR == 2 { print $2, $3, $4, $5, $6 }' || true

    printf "__TTERM_LOADAVG__ "
    awk '{ print $1, $2, $3 }' /proc/loadavg 2>/dev/null || true

    printf "__TTERM_UPTIME__ "
    awk '{ printf "%.0f\n", $1 }' /proc/uptime 2>/dev/null || true

    printf "__TTERM_NETWORK__ "
    network_interface="$(ip -o route show default 2>/dev/null | awk 'NR == 1 { for (i = 1; i <= NF; i++) if ($i == "dev") { print $(i + 1); exit } }')"
    if [ -z "$network_interface" ]; then
      network_interface="$(awk -F: '$1 !~ /^[[:space:]]*lo[[:space:]]*$/ { gsub(/[[:space:]]/, "", $1); print $1; exit }' /proc/net/dev 2>/dev/null)"
    fi
    if [ -n "$network_interface" ]; then
      awk -v iface="$network_interface" '
        {
          gsub(":", " ", $0)
        }
        $1 == iface {
          print iface, $2, $10, $4, $12, $5, $13
          exit
        }
      ' /proc/net/dev 2>/dev/null || true
    fi
"#;

pub(crate) fn parse_metrics_output(output: &str) -> ServerMetricsSnapshot {
    let os_release = extract_block(
        output,
        "__TTERM_OS_RELEASE_BEGIN__",
        "__TTERM_OS_RELEASE_END__",
    );
    let uname = find_prefixed_line(output, "__TTERM_UNAME__").unwrap_or_default();
    let kernel = (!uname.trim().is_empty()).then(|| uname.trim().to_string());
    let supported = uname.split_whitespace().next() == Some("Linux");

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
        disk: find_prefixed_line(output, "__TTERM_DF_ROOT__")
            .and_then(|line| parse_disk_metrics(&line)),
        network: find_prefixed_line(output, "__TTERM_NETWORK__")
            .and_then(|line| parse_network_metrics(&line)),
        load_average: find_prefixed_line(output, "__TTERM_LOADAVG__")
            .and_then(|line| parse_load_average(&line)),
        uptime_secs: find_prefixed_line(output, "__TTERM_UPTIME__")
            .and_then(|line| line.parse::<u64>().ok()),
        network_latency_ms: None,
    }
}

fn extract_block<'a>(output: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_idx = output.find(start)? + start.len();
    let rest = &output[start_idx..];
    let end_idx = rest.find(end)?;
    Some(rest[..end_idx].trim())
}

fn find_prefixed_line(output: &str, prefix: &str) -> Option<String> {
    output.lines().find_map(|line| {
        line.strip_prefix(prefix)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn parse_os_release(content: &str) -> Option<LinuxDistributionInfo> {
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

fn parse_memory_metrics(line: &str) -> Option<MemoryMetrics> {
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

fn parse_primary_ip(line: &str) -> Option<String> {
    line.split_whitespace()
        .find(|value| !value.starts_with("127.") && !value.starts_with("169.254."))
        .map(str::to_string)
}

fn parse_disk_metrics(line: &str) -> Option<DiskMetrics> {
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
        mount,
        total_kib: total,
        used_kib: used,
        available_kib: available,
        used_percent,
    })
}

fn parse_network_metrics(line: &str) -> Option<NetworkMetrics> {
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

#[cfg(test)]
mod tests {
    use super::{
        parse_load_average, parse_memory_metrics, parse_metrics_output, parse_network_metrics,
    };

    #[test]
    fn parses_memory_and_swap_usage() {
        let metrics =
            parse_memory_metrics("MemTotal=8192 MemAvailable=4096 SwapTotal=2048 SwapFree=1536")
                .unwrap();

        assert_eq!(metrics.used_kib, 4096);
        assert_eq!(metrics.swap_used_kib, 512);
        assert_eq!(metrics.swap_used_percent, 25.0);
    }

    #[test]
    fn missing_swap_is_reported_as_unconfigured() {
        let metrics = parse_memory_metrics("MemTotal=8192 MemAvailable=4096").unwrap();

        assert_eq!(metrics.swap_total_kib, 0);
        assert_eq!(metrics.swap_used_kib, 0);
        assert_eq!(metrics.swap_used_percent, 0.0);
    }

    #[test]
    fn parses_network_counters() {
        let metrics = parse_network_metrics("eth0 1024 2048 2 3 4 5").unwrap();

        assert_eq!(metrics.interface, "eth0");
        assert_eq!(metrics.received_bytes, 1024);
        assert_eq!(metrics.transmitted_bytes, 2048);
        assert_eq!(metrics.receive_errors, 2);
        assert_eq!(metrics.transmit_dropped, 5);
    }

    #[test]
    fn parses_load_average() {
        let load = parse_load_average("0.12 0.34 0.56").unwrap();

        assert_eq!(load.one, 0.12);
        assert_eq!(load.fifteen, 0.56);
    }

    #[test]
    fn snapshot_includes_extended_metrics() {
        let snapshot = parse_metrics_output(
            "__TTERM_UNAME__ Linux 6.8 x86_64\n__TTERM_NETWORK__ eth0 10 20 0 0 0 0\n__TTERM_LOADAVG__ 1.0 0.8 0.4\n__TTERM_UPTIME__ 3600",
        );

        assert!(snapshot.supported);
        assert_eq!(snapshot.network.unwrap().interface, "eth0");
        assert_eq!(snapshot.load_average.unwrap().five, 0.8);
        assert_eq!(snapshot.uptime_secs, Some(3600));
    }
}
