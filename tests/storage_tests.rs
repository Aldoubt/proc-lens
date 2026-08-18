use std::path::PathBuf;

use proc_lens::collector::storage::{StorageSnapshot, storage_for_path};

#[test]
fn storage_snapshot_reports_used_available_and_percent() {
    let snapshot = StorageSnapshot {
        target: PathBuf::from("/home/yangxuan"),
        total_bytes: 1_000,
        free_bytes: 100,
        available_bytes: 80,
    };

    assert_eq!(snapshot.used_bytes(), 900);
    assert_eq!(snapshot.available_bytes, 80);
    assert!((snapshot.used_percent() - 90.0).abs() < f32::EPSILON);
}

#[test]
fn zero_sized_storage_reports_zero_percent() {
    let snapshot = StorageSnapshot {
        target: PathBuf::from("/home/yangxuan"),
        total_bytes: 0,
        free_bytes: 0,
        available_bytes: 0,
    };

    assert_eq!(snapshot.used_bytes(), 0);
    assert_eq!(snapshot.used_percent(), 0.0);
}

#[test]
fn storage_for_path_reads_the_backing_filesystem() {
    let target = std::env::temp_dir();
    let snapshot = storage_for_path(&target).expect("statvfs should work for temp dir");

    assert_eq!(snapshot.target, target);
    assert!(snapshot.total_bytes > 0);
    assert!(snapshot.free_bytes <= snapshot.total_bytes);
    assert!(snapshot.available_bytes <= snapshot.free_bytes);
}
