use std::path::PathBuf;

use proc_lens::collector::storage::StorageSnapshot;

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
