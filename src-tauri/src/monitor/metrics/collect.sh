# Prints the host's metrics between __TTERM_*__ markers for
# monitor::metrics::parse_metrics_output. It runs under `sh -s` with the
# script on standard input, so everything happens inside collect_metrics,
# called with standard input from /dev/null: sh has read the whole script by
# then, and no command can swallow the rest of it.
#
# A missing tool or file leaves its section empty rather than failing the
# whole sample, so there is no `set -e`.
set -fu
PATH="$PATH:/sbin:/usr/sbin"

linux_metrics() {
  printf "__TTERM_PROC_STAT__ "
  awk '
    /^cpu / {
      print $2, $3, $4, $5, $6, $7, $8, $9, $10, $11
    }
  ' /proc/stat 2>/dev/null

  printf "__TTERM_PROC_STAT_CORES__ "
  awk '
    /^cpu[0-9]+ / {
      times = $2 "," $3 "," $4 "," $5 "," $6 "," $7 "," $8 "," $9 "," $10 "," $11
      printf "%s=%s ", $1, times
    }
    END {
      printf "\n"
    }
  ' /proc/stat 2>/dev/null

  printf "__TTERM_MEMINFO__ "
  awk '
    /^(MemTotal|MemAvailable|MemFree|Buffers|Cached|SReclaimable|SwapTotal|SwapFree):/ {
      gsub(":", "", $1)
      printf "%s=%s ", $1, $2
    }
    END {
      printf "\n"
    }
  ' /proc/meminfo 2>/dev/null

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
  df -P -k / 2>/dev/null | awk 'NR == 2 { print $2, $3, $4, $5, $6 }'

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
  '
  echo "__TTERM_DISKS_END__"

  printf "__TTERM_LOADAVG__ "
  awk '{ print $1, $2, $3 }' /proc/loadavg 2>/dev/null

  printf "__TTERM_UPTIME__ "
  awk '{ printf "%.0f\n", $1 }' /proc/uptime 2>/dev/null

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
    ' /proc/net/dev 2>/dev/null
  fi
}

# What FreeBSD and macOS report the same way: the default route's interface
# and its counters, the file systems, and the clock for the uptime.
bsd_common_metrics() {
  interface="$(route -n get default 2>/dev/null | awk '$1 == "interface:" { print $2; exit }')"

  printf "__TTERM_PRIMARY_IP__ "
  if [ -n "$interface" ]; then
    ifconfig "$interface" 2>/dev/null | awk '$1 == "inet" { print $2; exit }'
  fi
  printf "\n"

  echo "__TTERM_NETSTAT_BEGIN__"
  if [ -n "$interface" ]; then
    netstat -ibn -I "$interface" 2>/dev/null
  fi
  echo "__TTERM_NETSTAT_END__"

  echo "__TTERM_DF_BEGIN__"
  df -P -k 2>/dev/null
  echo "__TTERM_DF_END__"

  echo "__TTERM_MOUNT_BEGIN__"
  mount 2>/dev/null
  echo "__TTERM_MOUNT_END__"

  printf "__TTERM_NOW__ "
  date +%s
}

freebsd_metrics() {
  echo "__TTERM_SYSCTL_BEGIN__"
  sysctl -i kern.cp_time kern.cp_times hw.pagesize hw.physmem \
    vm.stats.vm.v_page_count vm.stats.vm.v_free_count vm.stats.vm.v_inactive_count \
    vm.stats.vm.v_cache_count kstat.zfs.misc.arcstats.size kstat.zfs.misc.arcstats.c_min \
    vm.loadavg kern.boottime 2>/dev/null
  echo "__TTERM_SYSCTL_END__"

  echo "__TTERM_SWAPINFO_BEGIN__"
  swapinfo -k 2>/dev/null
  echo "__TTERM_SWAPINFO_END__"

  bsd_common_metrics
}

darwin_metrics() {
  echo "__TTERM_SW_VERS_BEGIN__"
  sw_vers 2>/dev/null
  echo "__TTERM_SW_VERS_END__"

  echo "__TTERM_SYSCTL_BEGIN__"
  sysctl hw.memsize hw.pagesize vm.loadavg kern.boottime vm.swapusage 2>/dev/null
  echo "__TTERM_SYSCTL_END__"

  echo "__TTERM_VM_STAT_BEGIN__"
  vm_stat 2>/dev/null
  echo "__TTERM_VM_STAT_END__"

  bsd_common_metrics

  # macOS keeps no CPU tick counters a shell can read, so iostat measures
  # the CPU over one second; top would do the same at almost a second of
  # CPU time per sample. It goes last, so the other values are not a
  # second older than the rest of the sample.
  echo "__TTERM_IOSTAT_BEGIN__"
  iostat -n 0 -c 2 -w 1 2>/dev/null
  echo "__TTERM_IOSTAT_END__"
}

collect_metrics() {
  echo "__TTERM_OS_RELEASE_BEGIN__"
  if [ -r /etc/os-release ]; then
    cat /etc/os-release
  fi
  echo "__TTERM_OS_RELEASE_END__"

  printf "__TTERM_UNAME__ "
  uname -srm 2>/dev/null

  case "$(uname -s 2>/dev/null)" in
    Linux) linux_metrics ;;
    FreeBSD) freebsd_metrics ;;
    Darwin) darwin_metrics ;;
  esac
}

collect_metrics </dev/null
exit 0
