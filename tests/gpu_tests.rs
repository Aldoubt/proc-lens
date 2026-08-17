use std::collections::{BTreeMap, HashMap};

use proc_lens::app::{merge_gpu_sample, AppSnapshot, EnrichedProcess};
use proc_lens::classifier::{Classification, Confidence, ProcessType};
use proc_lens::gpu::{GpuDeviceSnapshot, GpuSample};
use proc_lens::process::{
    MemorySnapshot, ProcessGpuUsage, ProcessIdentity, ProcessSnapshot,
};

fn process(pid: i32) -> EnrichedProcess {
    EnrichedProcess {
        snapshot: ProcessSnapshot {
            identity: ProcessIdentity {
                pid,
                start_time_ticks: 10,
            },
            pid,
            ppid: 1,
            name: "fast_livo".into(),
            executable: None,
            cwd: None,
            command: vec!["fast_livo".into()],
            cgroup: Vec::new(),
            environment: BTreeMap::new(),
            cpu_percent: 50.0,
            memory_bytes: 256 * 1024 * 1024,
            gpu: None,
        },
        classification: Classification {
            process_type: ProcessType::Ros2,
            confidence: Confidence::High,
            score: 150,
            evidence: Vec::new(),
        },
        project: None,
        parent_chain: Vec::new(),
        tree_depth: 0,
        tree_order: 0,
    }
}

#[test]
fn gpu_sample_merges_vram_without_inventing_utilization() {
    let mut snapshot = AppSnapshot {
        cpu_percent: 42.0,
        memory: MemorySnapshot {
            total_bytes: 16 * 1024 * 1024 * 1024,
            available_bytes: 8 * 1024 * 1024 * 1024,
        },
        load_average: [0.1, 0.2, 0.3],
        gpu: None,
        processes: vec![process(42)],
    };

    let mut processes = HashMap::new();
    processes.insert(
        42,
        ProcessGpuUsage {
            gpu_index: Some(0),
            vram_bytes: Some(512 * 1024 * 1024),
            utilization_percent: None,
        },
    );
    let sample = GpuSample {
        devices: vec![GpuDeviceSnapshot {
            index: 0,
            name: Some("NVIDIA GPU".into()),
            utilization_percent: Some(61.0),
            memory_used_bytes: Some(2 * 1024 * 1024 * 1024),
            memory_total_bytes: Some(8 * 1024 * 1024 * 1024),
            temperature_c: Some(62),
            power_watts: Some(85.5),
        }],
        processes,
    };

    merge_gpu_sample(&mut snapshot, sample);

    let usage = snapshot.processes[0].snapshot.gpu.as_ref().unwrap();
    assert_eq!(usage.vram_bytes, Some(512 * 1024 * 1024));
    assert_eq!(usage.utilization_percent, None);
    assert_eq!(snapshot.gpu.as_ref().unwrap().devices[0].utilization_percent, Some(61.0));
}

#[test]
fn gpu_sample_does_not_attach_usage_to_unrelated_pid() {
    let mut snapshot = AppSnapshot {
        cpu_percent: 0.0,
        memory: MemorySnapshot {
            total_bytes: 1,
            available_bytes: 1,
        },
        load_average: [0.0, 0.0, 0.0],
        gpu: None,
        processes: vec![process(42)],
    };

    let mut processes = HashMap::new();
    processes.insert(
        99,
        ProcessGpuUsage {
            gpu_index: Some(0),
            vram_bytes: Some(123),
            utilization_percent: Some(75.0),
        },
    );

    merge_gpu_sample(
        &mut snapshot,
        GpuSample {
            devices: Vec::new(),
            processes,
        },
    );

    assert!(snapshot.processes[0].snapshot.gpu.is_none());
}

#[test]
fn textual_snapshot_reports_global_gpu_and_process_vram_without_fake_gpu_percent() {
    let mut snapshot = AppSnapshot {
        cpu_percent: 42.0,
        memory: MemorySnapshot {
            total_bytes: 16 * 1024 * 1024 * 1024,
            available_bytes: 8 * 1024 * 1024 * 1024,
        },
        load_average: [0.1, 0.2, 0.3],
        gpu: None,
        processes: vec![process(42)],
    };
    let mut processes = HashMap::new();
    processes.insert(
        42,
        ProcessGpuUsage {
            gpu_index: Some(0),
            vram_bytes: Some(512 * 1024 * 1024),
            utilization_percent: None,
        },
    );
    merge_gpu_sample(
        &mut snapshot,
        GpuSample {
            devices: vec![GpuDeviceSnapshot {
                index: 0,
                name: Some("RTX test".into()),
                utilization_percent: Some(61.0),
                memory_used_bytes: Some(2 * 1024 * 1024 * 1024),
                memory_total_bytes: Some(8 * 1024 * 1024 * 1024),
                temperature_c: Some(62),
                power_watts: Some(85.5),
            }],
            processes,
        },
    );

    let summary = proc_lens::app::format_snapshot(&snapshot, None);
    let detail = proc_lens::app::format_inspect(&snapshot, 42).unwrap();

    assert!(summary.contains("GPU0 61.0%"));
    assert!(summary.contains("VRAM 2.00 GiB / 8.00 GiB"));
    assert!(summary.contains("512.0M"));
    assert!(detail.contains("GPU        : -"));
    assert!(detail.contains("VRAM       : 512.00 MiB"));
}
