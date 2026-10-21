# Prints the host's metrics between __TTERM_*__ markers for
# monitor::metrics::parse_metrics_output.
set -efu

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

echo "__TTERM_DISKS_BEGIN__"
df -P -k -T 2>/dev/null | awk '
  NR > 1 {
    type = $2
    if (type ~ /^(tmpfs|devtmpfs|overlay|squashfs|proc|sysfs|cgroup|cgroup2|pstore|securityfs|debugfs|tracefs|configfs|fusectl|mqueue|hugetlbfs|ramfs|autofs|binfmt_misc|rpc_pipefs|nsfs)$/) {
      next
    }
    mount = $7
    for (i = 8; i <= NF; i++) {
      mount = mount " " $i
    }
    printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\n", $1, type, $3, $4, $5, $6, mount
  }
' || true
echo "__TTERM_DISKS_END__"

printf "__TTERM_LOADAVG__ "
awk '{ print $1, $2, $3 }' /proc/loadavg 2>/dev/null || true

printf "__TTERM_UPTIME__ "
awk '{ printf "%.0f\n", $1 }' /proc/uptime 2>/dev/null || true

printf "__TTERM_NETWORK__ "
network_interface="$(ip -o route show default 2>/dev/null | awk 'NR == 1 { for (i = 1; i <= NF; i++) if ($i == "dev") { print $(i + 1); exit } }')"
if [ -z "$network_interface" ]; then
  # The first two lines of /proc/net/dev are its header.
  network_interface="$(awk -F: 'NR > 2 && $1 !~ /^[[:space:]]*lo[[:space:]]*$/ { gsub(/[[:space:]]/, "", $1); print $1; exit }' /proc/net/dev 2>/dev/null)"
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
