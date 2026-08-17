use proc_lens::collector::cpu::{cpu_percent, parse_system_cpu_ticks};
use proc_lens::collector::memory::{parse_meminfo, parse_status_rss_bytes};
use proc_lens::collector::process::parse_process_stat;

#[test]
fn process_stat_handles_spaces_and_parentheses_in_comm() {
    let input = "321 (worker (camera) node) S 12 0 0 0 0 0 0 0 0 0 100 25 4 1 20 0 1 0 999 4096 42";
    let stat = parse_process_stat(input).expect("stat should parse");

    assert_eq!(stat.pid, 321);
    assert_eq!(stat.name, "worker (camera) node");
    assert_eq!(stat.ppid, 12);
    assert_eq!(stat.utime_ticks, 100);
    assert_eq!(stat.stime_ticks, 25);
    assert_eq!(stat.start_time_ticks, 999);
    assert_eq!(stat.vsize_bytes, 4096);
    assert_eq!(stat.rss_pages, 42);
}

#[test]
fn process_stat_rejects_truncated_input() {
    let err = parse_process_stat("10 (tiny) S 1 2 3").expect_err("truncated stat must fail");
    assert!(err.to_string().contains("field"));
}

#[test]
fn system_cpu_parser_counts_logical_cpus_and_total_ticks() {
    let input = concat!(
        "cpu  100 10 30 400 20 0 5 0 0 0\n",
        "cpu0 50 5 15 200 10 0 2 0 0 0\n",
        "cpu1 50 5 15 200 10 0 3 0 0 0\n",
        "intr 0\n",
    );
    let ticks = parse_system_cpu_ticks(input).expect("cpu ticks should parse");

    assert_eq!(ticks.total_ticks, 565);
    assert_eq!(ticks.idle_ticks, 420);
    assert_eq!(ticks.cpu_count, 2);
}

#[test]
fn cpu_percent_scales_to_logical_cpu_capacity() {
    let percent = cpu_percent(1000, 1200, 20, 70, 4).expect("delta is valid");
    assert!((percent - 100.0).abs() < f32::EPSILON);
}

#[test]
fn cpu_percent_returns_none_for_non_increasing_system_ticks() {
    assert_eq!(cpu_percent(1000, 1000, 20, 70, 4), None);
}

#[test]
fn meminfo_uses_mem_available() {
    let input = "MemTotal:       1000 kB\nMemFree:         100 kB\nMemAvailable:    400 kB\n";
    let mem = parse_meminfo(input).expect("meminfo should parse");

    assert_eq!(mem.total_bytes, 1_024_000);
    assert_eq!(mem.available_bytes, 409_600);
    assert_eq!(mem.used_bytes(), 614_400);
}

#[test]
fn status_rss_is_converted_from_kib_to_bytes() {
    let input = "Name:\ttest\nVmRSS:\t  2048 kB\nThreads:\t1\n";
    assert_eq!(parse_status_rss_bytes(input).unwrap(), Some(2_097_152));
}

#[test]
fn status_without_rss_is_not_an_error() {
    assert_eq!(parse_status_rss_bytes("Name:\tkthread\n").unwrap(), None);
}
