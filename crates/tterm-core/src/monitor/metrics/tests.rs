use super::bsd::{parse_disks, parse_netstat};
use super::linux::{parse_disk_metrics_block, parse_memory_metrics, parse_network_metrics};
use super::{parse_load_average, parse_metrics_output, parse_os_identity};

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
fn parses_bsd_load_average_in_braces() {
    let load = parse_load_average("{ 0.12 0.34 0.56 }").unwrap();

    assert_eq!(load.one, 0.12);
    assert_eq!(load.fifteen, 0.56);
}

#[test]
fn parses_and_orders_real_filesystems() {
    let disks = parse_disk_metrics_block(
        "/dev/mapper/vgdata-data\txfs\t104857600\t40894464\t63963136\t39%\t/data\n/dev/mapper/centos-root\txfs\t40894464\t18874368\t22020096\t47%\t/",
    );

    assert_eq!(disks.len(), 2);
    assert_eq!(disks[0].mount, "/");
    assert_eq!(disks[1].mount, "/data");
    assert_eq!(disks[1].filesystem_type, "xfs");
    assert_eq!(disks[1].used_percent, 39.0);
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

#[test]
fn an_unknown_system_is_unsupported_but_named() {
    let snapshot = parse_metrics_output("__TTERM_UNAME__ OpenBSD 7.6 amd64\n");

    assert!(!snapshot.supported);
    assert_eq!(snapshot.kernel.as_deref(), Some("OpenBSD 7.6 amd64"));
    assert!(snapshot.memory.is_none());
}

const DARWIN_OUTPUT: &str = "\
__TTERM_OS_RELEASE_BEGIN__
__TTERM_OS_RELEASE_END__
__TTERM_UNAME__ Darwin 24.1.0 arm64
__TTERM_SW_VERS_BEGIN__
ProductName:\t\tmacOS
ProductVersion:\t\t15.1
BuildVersion:\t\t24B83
__TTERM_SW_VERS_END__
__TTERM_SYSCTL_BEGIN__
hw.memsize: 17179869184
hw.pagesize: 16384
vm.loadavg: { 3.40 4.26 4.57 }
kern.boottime: { sec = 1700000000, usec = 267241 } Tue Nov 14 22:13:20 2023
vm.swapusage: total = 2048.00M  used = 512.00M  free = 1536.00M  (encrypted)
__TTERM_SYSCTL_END__
__TTERM_VM_STAT_BEGIN__
Mach Virtual Memory Statistics: (page size of 16384 bytes)
Pages free:                                    69871.
Pages active:                                 470924.
Pages wired down:                             131072.
Pages purgeable:                               65536.
\"Translation faults\":                     1275668011.
File-backed pages:                            322014.
Anonymous pages:                              327680.
Pages occupied by compressor:                  65536.
__TTERM_VM_STAT_END__
__TTERM_PRIMARY_IP__ 192.168.1.20

__TTERM_NETSTAT_BEGIN__
Name       Mtu   Network       Address            Ipkts Ierrs     Ibytes    Opkts Oerrs     Obytes  Coll
en0        1500  <Link#11>   aa:bb:cc:dd:ee:ff 10559147     3 9770299587  7528379     4 2855952338     0
en0        1500  192.168.1/24  192.168.1.20    10559147     - 9770299587  7528379     - 2855952338     -
__TTERM_NETSTAT_END__
__TTERM_DF_BEGIN__
Filesystem         1024-blocks      Used  Available Capacity  Mounted on
/dev/disk3s1s1       482797652  10500000 300000000     4%    /
devfs                      241       241          0   100%    /dev
/dev/disk3s5         482797652 170000000 300000000    37%    /System/Volumes/Data
map auto_home                0         0          0   100%    /System/Volumes/Data/home
/dev/disk5s1         975000000 500000000 475000000    52%    /Volumes/Backup Disk
__TTERM_DF_END__
__TTERM_MOUNT_BEGIN__
/dev/disk3s1s1 on / (apfs, sealed, local, read-only, journaled)
devfs on /dev (devfs, local, nobrowse)
/dev/disk3s5 on /System/Volumes/Data (apfs, local, journaled, nobrowse, root data)
map auto_home on /System/Volumes/Data/home (autofs, automounted, nobrowse)
/dev/disk5s1 on /Volumes/Backup Disk (hfs, local, nodev, nosuid, journaled)
__TTERM_MOUNT_END__
__TTERM_NOW__ 1700003600
__TTERM_IOSTAT_BEGIN__
      cpu    load average
 us sy id   1m   5m   15m
 14  8 78  3.40 4.26 4.57
 19  8 73  3.40 4.26 4.57
__TTERM_IOSTAT_END__
";

#[test]
fn reads_a_macos_host() {
    let snapshot = parse_metrics_output(DARWIN_OUTPUT);

    assert!(snapshot.supported);
    let distribution = snapshot.distribution.unwrap();
    assert_eq!(distribution.id, "macos");
    assert_eq!(distribution.pretty_name, "macOS 15.1");
    assert_eq!(snapshot.kernel.as_deref(), Some("Darwin 24.1.0 arm64"));
    // The second iostat row, not the average since boot.
    assert_eq!(snapshot.cpu_percent, Some(27.0));
    assert!(snapshot.cpu_times.is_none());
    assert_eq!(snapshot.primary_ip.as_deref(), Some("192.168.1.20"));
    assert_eq!(snapshot.uptime_secs, Some(3600));
    assert_eq!(snapshot.load_average.unwrap().fifteen, 4.57);

    // App memory (5 GiB anonymous - 1 GiB purgeable) + 2 GiB wired + 1 GiB compressed.
    let memory = snapshot.memory.unwrap();
    assert_eq!(memory.total_kib, 16 * 1024 * 1024);
    assert_eq!(memory.used_kib, 7 * 1024 * 1024);
    assert_eq!(memory.swap_total_kib, 2048 * 1024);
    assert_eq!(memory.swap_used_kib, 512 * 1024);

    let network = snapshot.network.unwrap();
    assert_eq!(network.interface, "en0");
    assert_eq!(network.received_bytes, 9_770_299_587);
    assert_eq!(network.transmitted_bytes, 2_855_952_338);
    assert_eq!(network.receive_errors, 3);
    assert_eq!(network.transmit_errors, 4);
}

#[test]
fn macos_disks_skip_hidden_volumes_and_count_the_apfs_container() {
    let snapshot = parse_metrics_output(DARWIN_OUTPUT);
    let mounts = snapshot
        .disks
        .iter()
        .map(|disk| disk.mount.as_str())
        .collect::<Vec<_>>();

    assert_eq!(mounts, ["/", "/Volumes/Backup Disk"]);
    let root = snapshot.disk.unwrap();
    assert_eq!(root.filesystem_type, "apfs");
    assert_eq!(root.used_kib, 482_797_652 - 300_000_000);
    assert!((root.used_percent - 37.86).abs() < 0.01);
    // Not APFS, so df's own figures stand.
    assert_eq!(snapshot.disks[1].used_kib, 500_000_000);
    assert_eq!(snapshot.disks[1].filesystem_type, "hfs");
}

#[test]
fn netstat_counters_are_matched_from_the_right() {
    // A tunnel has no link address, so its row is one word short.
    let network = parse_netstat(
        "Name       Mtu   Network       Address            Ipkts Ierrs     Ibytes    Opkts Oerrs     Obytes  Coll\n\
         utun4      1380  <Link#22>                          100     0      20000      200     0      40000     0\n",
    )
    .unwrap();

    assert_eq!(network.interface, "utun4");
    assert_eq!(network.received_bytes, 20000);
    assert_eq!(network.transmitted_bytes, 40000);
}

const FREEBSD_OUTPUT: &str = "\
__TTERM_OS_RELEASE_BEGIN__
NAME=FreeBSD
VERSION=\"14.1-RELEASE\"
VERSION_ID=\"14.1\"
ID=freebsd
ANSI_COLOR=\"0;31\"
PRETTY_NAME=\"FreeBSD 14.1-RELEASE\"
__TTERM_OS_RELEASE_END__
__TTERM_UNAME__ FreeBSD 14.1-RELEASE amd64
__TTERM_SYSCTL_BEGIN__
kern.cp_time: 1000 10 500 90 8400
kern.cp_times: 600 5 300 50 4045 400 5 200 40 4355
hw.pagesize: 4096
hw.physmem: 4250000000
vm.stats.vm.v_page_count: 1000000
vm.stats.vm.v_free_count: 200000
vm.stats.vm.v_inactive_count: 100000
vm.stats.vm.v_cache_count: 0
kstat.zfs.misc.arcstats.size: 819200000
kstat.zfs.misc.arcstats.c_min: 409600000
vm.loadavg: { 0.10 0.20 0.30 }
kern.boottime: { sec = 1700000000, usec = 0 } Tue Nov 14 22:13:20 2023
__TTERM_SYSCTL_END__
__TTERM_SWAPINFO_BEGIN__
Device          1K-blocks     Used    Avail Capacity
/dev/ada0p3       2097152   524288  1572864    25%
__TTERM_SWAPINFO_END__
__TTERM_PRIMARY_IP__ 10.0.2.15

__TTERM_NETSTAT_BEGIN__
Name    Mtu Network       Address              Ipkts Ierrs Idrop     Ibytes    Opkts Oerrs     Obytes  Coll
vtnet0 1500 <Link#1>      52:54:00:12:34:56    12345     1     2    1234567     2345     3     345678     0
vtnet0    - 10.0.2.0/24   10.0.2.15            12000     -     -    1200000     2300     -     340000     -
__TTERM_NETSTAT_END__
__TTERM_DF_BEGIN__
Filesystem         1024-blocks    Used    Avail Capacity  Mounted on
zroot/ROOT/default    20000000 2000000 18000000    10%    /
devfs                        1       1        0   100%    /dev
/dev/gpt/efiboot0       266176    1872   264304     1%    /boot/efi
zroot/home            18500000  500000 18000000     3%    /home
zroot                 18000096      96 18000000     0%    /zroot
zroot/var/log         18001000    1000 18000000     0%    /var/log
__TTERM_DF_END__
__TTERM_MOUNT_BEGIN__
zroot/ROOT/default on / (zfs, local, noatime, nfsv4acls)
devfs on /dev (devfs)
/dev/gpt/efiboot0 on /boot/efi (msdosfs, local)
zroot/home on /home (zfs, local, noatime, nfsv4acls)
zroot on /zroot (zfs, local, noatime, nfsv4acls)
zroot/var/log on /var/log (zfs, local, noatime, noexec, nosuid, nfsv4acls)
__TTERM_MOUNT_END__
__TTERM_NOW__ 1700086400
";

#[test]
fn reads_a_freebsd_host() {
    let snapshot = parse_metrics_output(FREEBSD_OUTPUT);

    assert!(snapshot.supported);
    let distribution = snapshot.distribution.unwrap();
    assert_eq!(distribution.id, "freebsd");
    assert_eq!(distribution.pretty_name, "FreeBSD 14.1-RELEASE");
    assert_eq!(snapshot.uptime_secs, Some(86400));
    assert_eq!(snapshot.load_average.unwrap().five, 0.2);
    assert_eq!(snapshot.primary_ip.as_deref(), Some("10.0.2.15"));

    let cpu = snapshot.cpu_times.unwrap();
    assert_eq!((cpu.user, cpu.nice, cpu.system), (1000, 10, 500));
    assert_eq!((cpu.irq, cpu.idle), (90, 8400));
    let cores = snapshot.cpu_core_times;
    assert_eq!(cores.len(), 2);
    assert_eq!(cores[1].id, "cpu1");
    assert_eq!(cores[1].times.user, 400);
    assert_eq!(cores[1].times.idle, 4355);

    // Free and inactive pages (1.2 GB) plus the ZFS cache above its minimum (400 MB).
    let memory = snapshot.memory.unwrap();
    assert_eq!(memory.total_kib, 4_096_000_000 / 1024);
    assert_eq!(memory.available_kib, 1_638_400_000 / 1024);
    assert_eq!(memory.swap_total_kib, 2_097_152);
    assert_eq!(memory.swap_free_kib, 1_572_864);

    let network = snapshot.network.unwrap();
    assert_eq!(network.interface, "vtnet0");
    assert_eq!(network.received_bytes, 1_234_567);
    assert_eq!(network.transmitted_bytes, 345_678);
    assert_eq!(network.receive_dropped, 2);
    assert_eq!(network.transmit_errors, 3);
}

#[test]
fn freebsd_zfs_datasets_fold_into_their_pool() {
    let snapshot = parse_metrics_output(FREEBSD_OUTPUT);

    assert_eq!(snapshot.disks.len(), 2);
    let pool = snapshot.disk.unwrap();
    assert_eq!(pool.filesystem, "zroot");
    assert_eq!(pool.mount, "/");
    assert_eq!(pool.used_kib, 2_000_000 + 500_000 + 96 + 1000);
    assert_eq!(pool.available_kib, 18_000_000);
    assert_eq!(pool.total_kib, pool.used_kib + pool.available_kib);
    assert_eq!(snapshot.disks[1].mount, "/boot/efi");
    assert_eq!(snapshot.disks[1].filesystem_type, "msdosfs");
}

#[test]
fn an_old_freebsd_is_named_from_uname() {
    let snapshot = parse_metrics_output("__TTERM_UNAME__ FreeBSD 12.4-RELEASE-p9 amd64\n");

    let distribution = snapshot.distribution.unwrap();
    assert_eq!(distribution.id, "freebsd");
    assert_eq!(distribution.pretty_name, "FreeBSD 12.4-RELEASE-p9");
    assert_eq!(distribution.version_id.as_deref(), Some("12.4"));
}

#[test]
fn df_rows_keep_spaces_in_devices_and_mount_points() {
    let disks = parse_disks(
        "Filesystem 1024-blocks Used Available Capacity Mounted on\n\
         //user@nas/share drive   1000   250   750    25%    /Volumes/share drive\n",
        "",
    );

    assert_eq!(disks.len(), 1);
    assert_eq!(disks[0].filesystem, "//user@nas/share drive");
    assert_eq!(disks[0].mount, "/Volumes/share drive");
    assert_eq!(disks[0].used_percent, 25.0);
}

#[test]
fn names_a_host_from_os_release_or_sw_vers() {
    let linux =
        parse_os_identity("ID=debian\nPRETTY_NAME=\"Debian GNU/Linux 12 (bookworm)\"\n").unwrap();
    assert_eq!(linux.id, "debian");

    let macos = parse_os_identity(
        "ProductName:\t\tmacOS\nProductVersion:\t\t15.1\nBuildVersion:\t\t24B83\n",
    )
    .unwrap();
    assert_eq!(macos.id, "macos");
    assert_eq!(macos.pretty_name, "macOS 15.1");

    assert!(parse_os_identity("").is_none());
}
