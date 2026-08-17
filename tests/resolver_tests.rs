use std::path::Path;

use proc_lens::process::resolver::resolve_ros2_install_path;

#[test]
fn resolves_colcon_install_layout() {
    let path = Path::new("/home/u/ws/install/fast_livo/lib/fast_livo/fast_livo");
    let identity = resolve_ros2_install_path(path).expect("ROS2 install path should resolve");

    assert_eq!(identity.workspace.as_deref(), Some(Path::new("/home/u/ws")));
    assert_eq!(identity.package.as_deref(), Some("fast_livo"));
    assert_eq!(identity.executable.as_deref(), Some("fast_livo"));
}

#[test]
fn rejects_install_path_when_lib_package_does_not_match() {
    let path = Path::new("/home/u/ws/install/fast_livo/lib/other_pkg/fast_livo");
    assert!(resolve_ros2_install_path(path).is_none());
}

#[test]
fn rejects_non_ros_install_path() {
    assert!(resolve_ros2_install_path(Path::new("/usr/bin/python3")).is_none());
}

#[test]
fn resolves_python_ros2_node_from_command_path_when_exe_is_interpreter() {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use proc_lens::process::{ProcessIdentity, ProcessSnapshot};
    use proc_lens::process::resolver::resolve_ros2_process;

    let process = ProcessSnapshot {
        identity: ProcessIdentity { pid: 42, start_time_ticks: 1 },
        pid: 42,
        ppid: 1,
        name: "python3".into(),
        executable: Some(PathBuf::from("/usr/bin/python3.10")),
        cwd: Some(PathBuf::from("/home/u/ws")),
        command: vec![
            "/home/u/ws/install/agt_mission_manager/lib/agt_mission_manager/mission_manager".into(),
            "--ros-args".into(),
        ],
        cgroup: Vec::new(),
        environment: BTreeMap::new(),
        cpu_percent: 0.0,
        memory_bytes: 0,
        gpu: None,
    };

    let identity = resolve_ros2_process(&process).expect("command path should resolve");
    assert_eq!(identity.workspace.as_deref(), Some(Path::new("/home/u/ws")));
    assert_eq!(identity.package.as_deref(), Some("agt_mission_manager"));
    assert_eq!(identity.executable.as_deref(), Some("mission_manager"));
}
