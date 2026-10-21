use super::linux::{parse_disk_metrics_block, parse_memory_metrics, parse_network_metrics};
use super::{parse_load_average, parse_metrics_output};

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
