use std::collections::BTreeMap;
use std::path::PathBuf;

use proc_lens::classifier::{Confidence, ProcessType, classify};
use proc_lens::process::{ProcessIdentity, ProcessSnapshot};

fn fixture(name: &str) -> ProcessSnapshot {
    ProcessSnapshot {
        identity: ProcessIdentity {
            pid: 200,
            start_time_ticks: 10,
        },
        pid: 200,
        ppid: 100,
        name: name.to_owned(),
        executable: None,
        cwd: None,
        command: vec![name.to_owned()],
        cgroup: Vec::new(),
        environment: BTreeMap::new(),
        cpu_percent: 0.0,
        memory_bytes: 0,
        gpu: None,
    }
}

#[test]
fn ros_args_produces_visible_ros2_evidence() {
    let mut process = fixture("fast_livo");
    process.command.push("--ros-args".into());

    let classification = classify(&process, &[]);

    assert_eq!(classification.process_type, ProcessType::Ros2);
    assert!(classification.score >= 70);
    assert!(classification.confidence >= Confidence::Medium);
    assert!(
        classification
            .evidence
            .iter()
            .any(|evidence| evidence.message.contains("--ros-args"))
    );
}

#[test]
fn ros_install_layout_beats_container_context_as_primary_type() {
    let mut process = fixture("fast_livo");
    process.executable = Some(PathBuf::from(
        "/home/u/ws/install/fast_livo/lib/fast_livo/fast_livo",
    ));
    process.command.push("--ros-args".into());
    process.cgroup.push("/docker/0123456789abcdef".into());

    let classification = classify(&process, &[]);

    assert_eq!(classification.process_type, ProcessType::Ros2);
    assert!(
        classification
            .evidence
            .iter()
            .any(|evidence| evidence.message.contains("install/<package>/lib"))
    );
}

#[test]
fn docker_cgroup_is_classified_as_container() {
    let mut process = fixture("python3");
    process
        .cgroup
        .push("/system.slice/docker-deadbeef.scope".into());

    assert_eq!(classify(&process, &[]).process_type, ProcessType::Container);
}

#[test]
fn systemd_service_context_is_explained() {
    let mut process = fixture("camera-daemon");
    process
        .cgroup
        .push("/system.slice/camera-daemon.service".into());

    let classification = classify(&process, &[]);
    assert_eq!(classification.process_type, ProcessType::Systemd);
    assert!(
        classification
            .evidence
            .iter()
            .any(|evidence| evidence.message.contains(".service"))
    );
}

#[test]
fn development_and_browser_processes_are_recognized() {
    assert_eq!(
        classify(&fixture("clangd"), &[]).process_type,
        ProcessType::Development
    );
    assert_eq!(
        classify(&fixture("firefox"), &[]).process_type,
        ProcessType::Browser
    );
}

#[test]
fn ros2_launch_command_is_classified_even_without_ros_args() {
    let mut process = fixture("python3");
    process.command = vec![
        "/usr/bin/python3".into(),
        "/opt/ros/humble/bin/ros2".into(),
        "launch".into(),
        "agt_bringup".into(),
        "navigation.launch.py".into(),
    ];

    let classification = classify(&process, &[]);

    assert_eq!(classification.process_type, ProcessType::Ros2);
    assert!(
        classification
            .evidence
            .iter()
            .any(|evidence| evidence.message.contains("ros2 launch/run"))
    );
}
