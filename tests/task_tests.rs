use std::collections::{BTreeMap, HashMap};

use proc_lens::app::{AppSnapshot, EnrichedProcess};
use proc_lens::classifier::{Classification, Confidence, ProcessType};
use proc_lens::collector::process::ProcessIoSnapshot;
use proc_lens::process::{MemorySnapshot, ProcessGpuUsage, ProcessIdentity, ProcessSnapshot};
use proc_lens::task::{TaskKind, aggregate_tasks};

fn process(
    pid: i32,
    ppid: i32,
    start_time_ticks: u64,
    name: &str,
    command: &[&str],
    cgroup: &[&str],
    process_type: ProcessType,
    cpu_percent: f32,
    memory_bytes: u64,
    parent_chain: &[i32],
) -> EnrichedProcess {
    EnrichedProcess {
        snapshot: ProcessSnapshot {
            identity: ProcessIdentity {
                pid,
                start_time_ticks,
            },
            pid,
            ppid,
            name: name.into(),
            executable: None,
            cwd: None,
            command: command.iter().map(|value| (*value).into()).collect(),
            cgroup: cgroup.iter().map(|value| (*value).into()).collect(),
            environment: BTreeMap::new(),
            cpu_percent,
            memory_bytes,
            gpu: None,
        },
        classification: Classification {
            process_type,
            confidence: Confidence::Medium,
            score: 80,
            evidence: Vec::new(),
        },
        project: None,
        parent_chain: parent_chain.to_vec(),
        tree_depth: parent_chain.len(),
        tree_order: pid as usize,
    }
}

fn snapshot(processes: Vec<EnrichedProcess>) -> AppSnapshot {
    AppSnapshot {
        cpu_percent: 50.0,
        memory: MemorySnapshot {
            total_bytes: 32 * 1024 * 1024 * 1024,
            available_bytes: 16 * 1024 * 1024 * 1024,
        },
        storage: None,
        load_average: [1.0, 0.8, 0.6],
        gpu: None,
        process_io: HashMap::new(),
        processes,
    }
}

#[test]
fn ros2_launcher_and_descendants_share_one_task() {
    let launcher = process(
        10,
        1,
        100,
        "ros2",
        &["ros2", "launch", "agt_navigation", "bringup.launch.py"],
        &[],
        ProcessType::Ros2,
        5.0,
        20,
        &[1],
    );
    let planner = process(
        11,
        10,
        101,
        "planner_server",
        &["planner_server", "--ros-args"],
        &[],
        ProcessType::Ros2,
        30.0,
        200,
        &[10, 1],
    );
    let controller = process(
        12,
        10,
        102,
        "controller_server",
        &["controller_server", "--ros-args"],
        &[],
        ProcessType::Ros2,
        40.0,
        300,
        &[10, 1],
    );

    let tasks = aggregate_tasks(&snapshot(vec![launcher, planner, controller]));

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].kind, TaskKind::Ros2Launch);
    assert_eq!(tasks[0].label, "agt_navigation/bringup.launch.py");
    assert_eq!(tasks[0].process_count, 3);
    assert_eq!(tasks[0].owner_pid, Some(10));
    assert_eq!(tasks[0].cpu_percent, 75.0);
    assert_eq!(tasks[0].rss_bytes, 520);
}

#[test]
fn independent_ros2_launchers_do_not_merge() {
    let first = process(
        20,
        1,
        200,
        "ros2",
        &["ros2", "launch", "pkg", "first.launch.py"],
        &[],
        ProcessType::Ros2,
        1.0,
        10,
        &[1],
    );
    let second = process(
        21,
        1,
        201,
        "ros2",
        &["ros2", "launch", "pkg", "second.launch.py"],
        &[],
        ProcessType::Ros2,
        1.0,
        10,
        &[1],
    );

    let tasks = aggregate_tasks(&snapshot(vec![first, second]));

    assert_eq!(tasks.len(), 2);
    assert_ne!(tasks[0].id, tasks[1].id);
}

#[test]
fn ros2_run_is_a_task_anchor() {
    let runner = process(
        30,
        1,
        300,
        "ros2",
        &["ros2", "run", "camera_pkg", "camera_node"],
        &[],
        ProcessType::Ros2,
        12.0,
        64,
        &[1],
    );

    let tasks = aggregate_tasks(&snapshot(vec![runner]));

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].kind, TaskKind::Ros2Launch);
    assert_eq!(tasks[0].label, "camera_pkg/camera_node");
    assert!(tasks[0].id.as_str().starts_with("ros2:30:300"));
}

#[test]
fn concrete_systemd_service_groups_members() {
    let service = process(
        40,
        1,
        400,
        "daemon",
        &["daemon"],
        &["/system.slice/inspection.service"],
        ProcessType::Systemd,
        5.0,
        100,
        &[1],
    );
    let worker = process(
        41,
        40,
        401,
        "worker",
        &["worker"],
        &["/system.slice/inspection.service"],
        ProcessType::Systemd,
        8.0,
        200,
        &[40, 1],
    );

    let tasks = aggregate_tasks(&snapshot(vec![service, worker]));

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].kind, TaskKind::SystemdService);
    assert_eq!(tasks[0].id.as_str(), "systemd:inspection.service");
    assert_eq!(tasks[0].process_count, 2);
}

#[test]
fn generic_user_manager_is_not_a_systemd_task_anchor() {
    let process = process(
        50,
        1,
        500,
        "desktop-child",
        &["desktop-child"],
        &["/user.slice/user-1000.slice/user@1000.service/app.slice/app.scope"],
        ProcessType::Generic,
        2.0,
        10,
        &[1],
    );

    let tasks = aggregate_tasks(&snapshot(vec![process]));

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].kind, TaskKind::Process);
    assert!(tasks[0].id.as_str().starts_with("proc:50:500"));
}

#[test]
fn different_container_cgroups_do_not_merge() {
    let first = process(
        60,
        1,
        600,
        "worker",
        &["worker"],
        &["/system.slice/docker-aaa.scope"],
        ProcessType::Container,
        3.0,
        100,
        &[1],
    );
    let second = process(
        61,
        1,
        601,
        "worker",
        &["worker"],
        &["/system.slice/docker-bbb.scope"],
        ProcessType::Container,
        4.0,
        100,
        &[1],
    );

    let tasks = aggregate_tasks(&snapshot(vec![first, second]));

    assert_eq!(tasks.len(), 2);
    assert_ne!(tasks[0].id, tasks[1].id);
}

#[test]
fn browser_owner_and_generic_child_share_application_task() {
    let browser = process(
        70,
        1,
        700,
        "firefox",
        &["firefox"],
        &[],
        ProcessType::Browser,
        5.0,
        100,
        &[1],
    );
    let child = process(
        71,
        70,
        701,
        "Isolated Web Co",
        &["Isolated Web Co"],
        &[],
        ProcessType::Generic,
        15.0,
        200,
        &[70, 1],
    );

    let tasks = aggregate_tasks(&snapshot(vec![browser, child]));

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].kind, TaskKind::Application);
    assert_eq!(tasks[0].owner_pid, Some(70));
    assert_eq!(tasks[0].process_count, 2);
    assert_eq!(tasks[0].label, "Firefox");
}

#[test]
fn unrelated_processes_with_same_name_remain_separate() {
    let first = process(
        80,
        1,
        800,
        "worker",
        &["worker"],
        &[],
        ProcessType::Generic,
        1.0,
        10,
        &[1],
    );
    let second = process(
        81,
        1,
        801,
        "worker",
        &["worker"],
        &[],
        ProcessType::Generic,
        1.0,
        10,
        &[1],
    );

    let tasks = aggregate_tasks(&snapshot(vec![first, second]));

    assert_eq!(tasks.len(), 2);
}

#[test]
fn pid_reuse_changes_pid_backed_task_id() {
    let first = aggregate_tasks(&snapshot(vec![process(
        90,
        1,
        900,
        "worker",
        &["worker"],
        &[],
        ProcessType::Generic,
        1.0,
        10,
        &[1],
    )]));
    let second = aggregate_tasks(&snapshot(vec![process(
        90,
        1,
        901,
        "worker",
        &["worker"],
        &[],
        ProcessType::Generic,
        1.0,
        10,
        &[1],
    )]));

    assert_ne!(first[0].id, second[0].id);
}

#[test]
fn task_cpu_can_exceed_one_hundred_and_rss_is_summed() {
    let launcher = process(
        100,
        1,
        1000,
        "ros2",
        &["ros2", "launch", "pkg", "heavy.launch.py"],
        &[],
        ProcessType::Ros2,
        80.0,
        512,
        &[1],
    );
    let child = process(
        101,
        100,
        1001,
        "node",
        &["node", "--ros-args"],
        &[],
        ProcessType::Ros2,
        65.0,
        1024,
        &[100, 1],
    );

    let task = aggregate_tasks(&snapshot(vec![launcher, child])).remove(0);

    assert_eq!(task.cpu_percent, 145.0);
    assert_eq!(task.rss_bytes, 1536);
}

#[test]
fn missing_member_io_makes_task_io_unknown() {
    let launcher = process(
        110,
        1,
        1100,
        "ros2",
        &["ros2", "launch", "pkg", "io.launch.py"],
        &[],
        ProcessType::Ros2,
        1.0,
        10,
        &[1],
    );
    let child = process(
        111,
        110,
        1101,
        "node",
        &["node", "--ros-args"],
        &[],
        ProcessType::Ros2,
        1.0,
        10,
        &[110, 1],
    );
    let launcher_identity = launcher.snapshot.identity;
    let mut snapshot = snapshot(vec![launcher, child]);
    snapshot.process_io.insert(
        launcher_identity,
        ProcessIoSnapshot {
            read_bytes: 100,
            write_bytes: 200,
            read_bytes_per_second: Some(10),
            write_bytes_per_second: Some(20),
        },
    );

    let task = aggregate_tasks(&snapshot).remove(0);

    assert_eq!(task.disk_read_bytes, None);
    assert_eq!(task.disk_write_bytes, None);
    assert_eq!(task.read_bytes_per_second, None);
    assert_eq!(task.write_bytes_per_second, None);
}

#[test]
fn task_io_is_summed_when_every_member_has_data() {
    let launcher = process(
        120,
        1,
        1200,
        "ros2",
        &["ros2", "launch", "pkg", "io.launch.py"],
        &[],
        ProcessType::Ros2,
        1.0,
        10,
        &[1],
    );
    let child = process(
        121,
        120,
        1201,
        "node",
        &["node", "--ros-args"],
        &[],
        ProcessType::Ros2,
        1.0,
        10,
        &[120, 1],
    );
    let launcher_identity = launcher.snapshot.identity;
    let child_identity = child.snapshot.identity;
    let mut snapshot = snapshot(vec![launcher, child]);
    for (identity, read, write, read_rate, write_rate) in [
        (launcher_identity, 100, 200, 10, 20),
        (child_identity, 300, 400, 30, 40),
    ] {
        snapshot.process_io.insert(
            identity,
            ProcessIoSnapshot {
                read_bytes: read,
                write_bytes: write,
                read_bytes_per_second: Some(read_rate),
                write_bytes_per_second: Some(write_rate),
            },
        );
    }

    let task = aggregate_tasks(&snapshot).remove(0);

    assert_eq!(task.disk_read_bytes, Some(400));
    assert_eq!(task.disk_write_bytes, Some(600));
    assert_eq!(task.read_bytes_per_second, Some(40));
    assert_eq!(task.write_bytes_per_second, Some(60));
}

#[test]
fn no_member_vram_does_not_become_zero() {
    let task = aggregate_tasks(&snapshot(vec![process(
        130,
        1,
        1300,
        "worker",
        &["worker"],
        &[],
        ProcessType::Generic,
        1.0,
        10,
        &[1],
    )]))
    .remove(0);

    assert_eq!(task.vram_bytes, None);
}

#[test]
fn known_member_vram_is_summed() {
    let mut launcher = process(
        140,
        1,
        1400,
        "ros2",
        &["ros2", "launch", "pkg", "gpu.launch.py"],
        &[],
        ProcessType::Ros2,
        1.0,
        10,
        &[1],
    );
    launcher.snapshot.gpu = Some(ProcessGpuUsage {
        gpu_index: Some(0),
        vram_bytes: Some(100),
        utilization_percent: Some(10.0),
    });
    let mut child = process(
        141,
        140,
        1401,
        "node",
        &["node", "--ros-args"],
        &[],
        ProcessType::Ros2,
        1.0,
        10,
        &[140, 1],
    );
    child.snapshot.gpu = Some(ProcessGpuUsage {
        gpu_index: Some(0),
        vram_bytes: Some(300),
        utilization_percent: Some(20.0),
    });

    let task = aggregate_tasks(&snapshot(vec![launcher, child])).remove(0);

    assert_eq!(task.vram_bytes, Some(400));
}
