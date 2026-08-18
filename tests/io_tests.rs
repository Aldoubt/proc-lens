use std::time::Duration;

use proc_lens::collector::process::{io_rate_bytes_per_second, parse_process_io};

#[test]
fn process_io_parser_uses_actual_block_io_bytes() {
    let parsed = parse_process_io(
        "rchar: 99999\n\
         wchar: 88888\n\
         syscr: 10\n\
         syscw: 20\n\
         read_bytes: 4096\n\
         write_bytes: 8192\n\
         cancelled_write_bytes: 1024\n",
    )
    .expect("valid /proc/<pid>/io");

    assert_eq!(parsed.read_bytes, 4096);
    assert_eq!(parsed.write_bytes, 8192);
}

#[test]
fn io_rate_uses_elapsed_wall_clock_time() {
    assert_eq!(
        io_rate_bytes_per_second(1_000, 5_096, Duration::from_secs(2)),
        Some(2_048)
    );
}

#[test]
fn io_rate_is_unknown_when_counter_moves_backwards() {
    assert_eq!(
        io_rate_bytes_per_second(5_000, 4_000, Duration::from_secs(1)),
        None
    );
}

#[test]
fn io_rate_is_unknown_for_zero_elapsed_time() {
    assert_eq!(io_rate_bytes_per_second(1_000, 2_000, Duration::ZERO), None);
}
