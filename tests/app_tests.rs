use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use proc_lens::app::{
    AppSnapshot, EnrichedProcess, format_inspect, format_snapshot, project_label,
};
use proc_lens::classifier::{Classification, Confidence, Evidence, ProcessType};
use proc_lens::collector::process::ProcessIoSnapshot;
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
        storage: None,
        load_average: [1.0, 0.8, 0.6],
        gpu: None,
        process_io: HashMap::new(),
        processes: vec![enriched()],
    }
}

fn browser_child_snapshot() -> AppSnapshot {
    let mut parent = enriched();
    parent.snapshot.identity = ProcessIdentity {
        pid: 100,
        start_time_ticks: 1000,
    };
    parent.snapshot.pid = 100;
    parent.snapshot.ppid = 1;
    parent.snapshot.name = "firefox".into();
    parent.snapshot.executable = Some(PathBuf::from("/usr/bin/firefox"));
    parent.snapshot.command = vec!["firefox".into()];
    parent.snapshot.cpu_percent = 5.0;
    parent.classification = Classification {
        process_type: ProcessType::Browser,
        confidence: Confidence::Medium,
        score: 80,
        evidence: Vec::new(),
    };
    parent.project = None;
    parent.parent_chain = vec![1];
    parent.tree_depth = 1;

    let mut child = enriched();
    child.snapshot.identity = ProcessIdentity {
        pid: 101,
        start_time_ticks: 1001,
    };
    child.snapshot.pid = 101;
    child.snapshot.ppid = 100;
    child.snapshot.name = "Isolated Web Co".into();
    child.snapshot.executable = Some(PathBuf::from("/usr/lib/firefox/firefox"));
    child.snapshot.command = vec!["Isolated Web Co".into()];
    child.snapshot.cpu_percent = 12.0;
    child.classification = Classification {
        process_type: ProcessType::Generic,
        confidence: Confidence::Low,
        score: 0,
        evidence: Vec::new(),
    };
    child.project = None;
    child.parent_chain = vec![100, 1];
    child.tree_depth = 2;
    child.tree_order = 1;

    AppSnapshot {
        cpu_percent: 20.0,
        memory: MemorySnapshot {
            total_bytes: 32 * 1024 * 1024 * 1024,
            available_bytes: 20 * 1024 * 1024 * 1024,
        },
        storage: None,
        load_average: [1.0, 0.8, 0.6],
        gpu: None,
        process_io: HashMap::new(),
        processes: vec![parent, child],
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
fn cli_snapshot_uses_inherited_browser_provenance() {
    let snapshot = browser_child_snapshot();
    let output = format_snapshot(&snapshot, Some(ProcessType::Browser));

    assert!(output.contains("101"));
    assert!(output.contains("BROWSER"));
    assert!(output.contains("Firefox"));
    assert!(output.contains("Isolated Web Co"));
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
fn cli_inspect_reports_process_disk_io_and_rates() {
    let mut snapshot = snapshot();
    let identity = snapshot.processes[0].snapshot.identity;
    snapshot.process_io.insert(
        identity,
        ProcessIoSnapshot {
            read_bytes: 8 * 1024 * 1024,
            write_bytes: 32 * 1024 * 1024,
            read_bytes_per_second: Some(512 * 1024),
            write_bytes_per_second: Some(2 * 1024 * 1024),
        },
    );

    let output = format_inspect(&snapshot, 18452).expect("pid exists");

    assert!(output.contains("Disk read"));
    assert!(output.contains("8.0M"));
    assert!(output.contains("Disk write"));
    assert!(output.contains("32.0M"));
    assert!(output.contains("Read rate"));
    assert!(output.contains("512.0K/s"));
    assert!(output.contains("Write rate"));
    assert!(output.contains("2.0M/s"));
}

#[test]
fn cli_inspect_separates_direct_type_from_inherited_provenance() {
    let snapshot = browser_child_snapshot();
    let output = format_inspect(&snapshot, 101).expect("pid exists");

    assert!(output.contains("Type       : PROCESS"));
    assert!(output.contains("Provenance"));
    assert!(output.contains("Owner PID   : 100"));
    assert!(output.contains("Owner       : firefox"));
    assert!(output.contains("Display type: BROWSER"));
    assert!(output.contains("Project     : Firefox"));
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

#[test]
fn development_project_label_uses_nearest_git_root() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("proc-lens-project-{nonce}"));
    let nested = root.join("src").join("deep");
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    fs::create_dir_all(&nested).expect("create nested cwd");

    let mut process = enriched();
    process.snapshot.name = "clangd".into();
    process.snapshot.cwd = Some(nested);
    process.project = None;
    process.classification.process_type = ProcessType::Development;

    assert_eq!(
        project_label(&process),
        root.file_name().unwrap().to_string_lossy()
    );

    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn browser_project_label_is_not_the_executable_name() {
    let mut process = enriched();
    process.snapshot.name = "firefox".into();
    process.project = None;
    process.classification.process_type = ProcessType::Browser;

    assert_eq!(project_label(&process), "-");
}
