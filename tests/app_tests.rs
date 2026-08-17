use std::collections::BTreeMap;
use std::path::PathBuf;

use proc_lens::app::{AppSnapshot, EnrichedProcess, format_inspect, format_snapshot};
use proc_lens::classifier::{Classification, Confidence, Evidence, ProcessType};
use proc_lens::process::{MemorySnapshot, ProcessIdentity, ProcessSnapshot, ProjectIdentity};

fn enriched() -> EnrichedProcess {
    let process = ProcessSnapshot {
        identity: ProcessIdentity {
            pid: 18452,
            start_time_ticks: 99,
        },
        pid: 18452,
        ppid: 100,
        name: "fast_livo".into(),
        executable: Some(PathBuf::from(
            "/home/u/agt_navigation_runtime/install/fast_livo/lib/fast_livo/fast_livo",
        )),
        cwd: Some(PathBuf::from("/home/u/agt_navigation_runtime")),
        command: vec!["fast_livo".into(), "--ros-args".into()],
        cgroup: vec!["/user.slice/user-1000.slice/session-2.scope".into()],
        environment: BTreeMap::new(),
        cpu_percent: 83.4,
        memory_bytes: 1_299_226_624,
        gpu: None,
    };

    EnrichedProcess {
        snapshot: process,
        classification: Classification {
            process_type: ProcessType::Ros2,
            confidence: Confidence::High,
            score: 150,
            evidence: vec![Evidence {
                process_type: ProcessType::Ros2,
                score: 70,
                message: "command contains --ros-args".into(),
            }],
        },
        project: Some(ProjectIdentity {
            workspace: Some(PathBuf::from("/home/u/agt_navigation_runtime")),
            package: Some("fast_livo".into()),
            executable: Some("fast_livo".into()),
        }),
        parent_chain: vec![100, 1],
        tree_depth: 2,
        tree_order: 0,
    }
}

fn snapshot() -> AppSnapshot {
    AppSnapshot {
        cpu_percent: 52.0,
        memory: MemorySnapshot {
            total_bytes: 32 * 1024 * 1024 * 1024,
            available_bytes: 20 * 1024 * 1024 * 1024,
        },
        load_average: [1.0, 0.8, 0.6],
        gpu: None,
        processes: vec![enriched()],
    }
}

#[test]
fn cli_snapshot_contains_process_identity_and_resources() {
    let output = format_snapshot(&snapshot(), None);

    assert!(output.contains("PID"));
    assert!(output.contains("TYPE"));
    assert!(output.contains("18452"));
    assert!(output.contains("ROS2"));
    assert!(output.contains("agt_navigation_runtime"));
    assert!(output.contains("83.4"));
    assert!(output.contains("fast_livo"));
}

#[test]
fn cli_snapshot_filters_by_process_type() {
    let output = format_snapshot(&snapshot(), Some(ProcessType::Browser));
    assert!(!output.contains("18452"));
}

#[test]
fn cli_inspect_explains_provenance_and_evidence() {
    let output = format_inspect(&snapshot(), 18452).expect("pid exists");

    assert!(output.contains("/home/u/agt_navigation_runtime/install/fast_livo"));
    assert!(output.contains("/home/u/agt_navigation_runtime"));
    assert!(output.contains("fast_livo --ros-args"));
    assert!(output.contains("Parent chain"));
    assert!(output.contains("100 -> 1"));
    assert!(output.contains("command contains --ros-args"));
}

#[test]
fn cli_inspect_names_parent_processes_when_they_are_still_visible() {
    let mut snapshot = snapshot();
    let mut parent = enriched();
    parent.snapshot.pid = 100;
    parent.snapshot.identity.pid = 100;
    parent.snapshot.name = "ros2".into();
    parent.snapshot.command = vec![
        "ros2".into(),
        "launch".into(),
        "agt_bringup".into(),
        "navigation.launch.py".into(),
    ];
    parent.parent_chain = vec![1];
    parent.tree_order = 1;
    snapshot.processes.push(parent);

    let output = format_inspect(&snapshot, 18452).expect("pid exists");

    assert!(output.contains("PID 100: ros2 launch agt_bringup navigation.launch.py"));
    assert!(output.contains("PID 1: <exited or inaccessible>"));
}
