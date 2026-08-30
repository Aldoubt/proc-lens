use std::collections::{BTreeMap, HashMap};

use proc_lens::app::{AppSnapshot, EnrichedProcess};
use proc_lens::classifier::{Classification, Confidence, ProcessType};
use proc_lens::process::{MemorySnapshot, ProcessIdentity, ProcessSnapshot};
use proc_lens::task::{TaskId, format_task, format_tasks};

fn process(pid: i32, start: u64, name: &str, cpu: f32, rss: u64) -> EnrichedProcess {
    EnrichedProcess {
        snapshot: ProcessSnapshot {
            identity: ProcessIdentity {
                pid,
                start_time_ticks: start,
            },
            pid,
            ppid: 1,
            name: name.into(),
            executable: None,
            cwd: None,
            command: vec![name.into()],
            cgroup: Vec::new(),
            environment: BTreeMap::new(),
            cpu_percent: cpu,
            memory_bytes: rss,
            gpu: None,
        },
        classification: Classification {
            process_type: ProcessType::Generic,
            confidence: Confidence::Low,
            score: 0,
            evidence: Vec::new(),
        },
        project: None,
        parent_chain: vec![1],
        tree_depth: 1,
        tree_order: pid as usize,
    }
}

fn snapshot() -> AppSnapshot {
    AppSnapshot {
        cpu_percent: 25.0,
        memory: MemorySnapshot {
            total_bytes: 8 * 1024 * 1024 * 1024,
            available_bytes: 4 * 1024 * 1024 * 1024,
        },
        storage: None,
        load_average: [0.5, 0.4, 0.3],
        gpu: None,
        process_io: HashMap::new(),
        processes: vec![
            process(20, 200, "slow", 5.0, 400),
            process(10, 100, "fast", 15.0, 200),
        ],
    }
}

#[test]
fn task_snapshot_has_explicit_rss_sum_header_and_cpu_order() {
    let output = format_tasks(&snapshot());

    assert!(output.contains("RSS Σ"));
    assert!(output.contains("TASK ID"));
    assert!(output.find("proc:10:100").unwrap() < output.find("proc:20:200").unwrap());
}

#[test]
fn task_detail_requires_exact_task_id() {
    let snapshot = snapshot();

    assert!(format_task(&snapshot, &TaskId::new("proc:10:100")).is_some());
    assert!(format_task(&snapshot, &TaskId::new("proc:10")).is_none());
    assert!(format_task(&snapshot, &TaskId::new("missing")).is_none());
}

#[test]
fn task_detail_labels_memory_as_summed_rss_and_unknown_io_as_dash() {
    let snapshot = snapshot();
    let output = format_task(&snapshot, &TaskId::new("proc:10:100")).unwrap();

    assert!(output.contains("RAM RSS Σ"));
    assert!(output.contains("Read rate  : -"));
    assert!(output.contains("Write rate : -"));
    assert!(output.contains("PID"));
    assert!(output.contains("fast"));
}
