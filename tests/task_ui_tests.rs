use std::collections::{BTreeMap, HashMap};

use proc_lens::app::{AppSnapshot, EnrichedProcess, EntityMode, UiState, ViewMode};
use proc_lens::classifier::{Classification, Confidence, ProcessType};
use proc_lens::process::{MemorySnapshot, ProcessIdentity, ProcessSnapshot};
use proc_lens::task::TaskId;
use proc_lens::ui::model::PresentationModel;

#[allow(clippy::too_many_arguments)]
fn process(
    pid: i32,
    ppid: i32,
    start: u64,
    name: &str,
    command: &[&str],
    process_type: ProcessType,
    cpu: f32,
    rss: u64,
    parent_chain: &[i32],
) -> EnrichedProcess {
    EnrichedProcess {
        snapshot: ProcessSnapshot {
            identity: ProcessIdentity {
                pid,
                start_time_ticks: start,
            },
            pid,
            ppid,
            name: name.into(),
            executable: None,
            cwd: None,
            command: command.iter().map(|value| (*value).into()).collect(),
            cgroup: Vec::new(),
            environment: BTreeMap::new(),
            cpu_percent: cpu,
            memory_bytes: rss,
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
        cpu_percent: 10.0,
        memory: MemorySnapshot {
            total_bytes: 1024,
            available_bytes: 512,
        },
        storage: None,
        load_average: [0.1, 0.2, 0.3],
        gpu: None,
        process_io: HashMap::new(),
        processes,
    }
}

fn launcher_tree(owner_cpu: f32, child_cpu: f32) -> AppSnapshot {
    snapshot(vec![
        process(
            10,
            1,
            100,
            "ros2",
            &["ros2", "launch", "agt_navigation", "bringup.launch.py"],
            ProcessType::Ros2,
            owner_cpu,
            100,
            &[1],
        ),
        process(
            11,
            10,
            101,
            "planner_server",
            &["planner_server", "--ros-args"],
            ProcessType::Ros2,
            child_cpu,
            200,
            &[10, 1],
        ),
    ])
}

#[test]
fn ui_defaults_to_process_mode_and_keeps_task_selection_separate() {
    let mut state = UiState::default();
    assert_eq!(state.entity_mode, EntityMode::Process);
    assert_eq!(state.selected_task_id, None);

    state.selected_pid = Some(42);
    state.selected_task_id = Some(TaskId::new("task-a"));
    state.entity_mode = EntityMode::Task;
    state.open_detail();

    assert_eq!(state.view, ViewMode::Detail);
    assert_eq!(state.selected_pid, Some(42));
    assert_eq!(state.selected_task_id, Some(TaskId::new("task-a")));
}

#[test]
fn task_rows_aggregate_members_and_ignore_process_type_filter() {
    let mut state = UiState {
        entity_mode: EntityMode::Task,
        process_type_filter: Some(ProcessType::Browser),
        ..UiState::default()
    };
    let model = PresentationModel::new(launcher_tree(10.0, 20.0), &mut state);
    let rows = model.visible_task_rows(&state);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].task.process_count, 2);
    assert_eq!(rows[0].task.label, "agt_navigation/bringup.launch.py");
    assert_eq!(rows[0].task.rss_bytes, 300);
}

#[test]
fn task_cpu_ema_is_keyed_by_task_id() {
    let mut state = UiState {
        entity_mode: EntityMode::Task,
        ..UiState::default()
    };
    let mut model = PresentationModel::new(launcher_tree(10.0, 10.0), &mut state);
    let id = TaskId::new("ros2:10:100");

    model.integrate_snapshot(launcher_tree(30.0, 30.0), &mut state, false);

    assert!((model.smoothed_cpu_for_task(&id).unwrap() - 34.0).abs() < 0.001);
}

#[test]
fn selected_task_survives_refresh_and_disappearance_falls_back() {
    let first = snapshot(vec![
        process(
            10,
            1,
            100,
            "first",
            &["first"],
            ProcessType::Generic,
            30.0,
            100,
            &[1],
        ),
        process(
            20,
            1,
            200,
            "second",
            &["second"],
            ProcessType::Generic,
            20.0,
            100,
            &[1],
        ),
        process(
            30,
            1,
            300,
            "third",
            &["third"],
            ProcessType::Generic,
            10.0,
            100,
            &[1],
        ),
    ]);
    let mut state = UiState {
        entity_mode: EntityMode::Task,
        selected_task_id: Some(TaskId::new("proc:20:200")),
        ..UiState::default()
    };
    let mut model = PresentationModel::new(first, &mut state);
    assert_eq!(model.selected_task_index(&state), Some(1));

    model.integrate_snapshot(
        snapshot(vec![
            process(
                10,
                1,
                100,
                "first",
                &["first"],
                ProcessType::Generic,
                30.0,
                100,
                &[1],
            ),
            process(
                20,
                1,
                200,
                "second",
                &["second"],
                ProcessType::Generic,
                40.0,
                100,
                &[1],
            ),
            process(
                30,
                1,
                300,
                "third",
                &["third"],
                ProcessType::Generic,
                10.0,
                100,
                &[1],
            ),
        ]),
        &mut state,
        true,
    );
    assert_eq!(state.selected_task_id, Some(TaskId::new("proc:20:200")));

    model.integrate_snapshot(
        snapshot(vec![
            process(
                10,
                1,
                100,
                "first",
                &["first"],
                ProcessType::Generic,
                30.0,
                100,
                &[1],
            ),
            process(
                30,
                1,
                300,
                "third",
                &["third"],
                ProcessType::Generic,
                10.0,
                100,
                &[1],
            ),
        ]),
        &mut state,
        true,
    );

    assert_eq!(state.selected_task_id, Some(TaskId::new("proc:30:300")));
    assert_eq!(model.selected_task_index(&state), Some(1));
}

#[test]
fn task_search_matches_member_command() {
    let mut state = UiState {
        entity_mode: EntityMode::Task,
        query: "planner_server".into(),
        ..UiState::default()
    };
    let model = PresentationModel::new(launcher_tree(10.0, 20.0), &mut state);

    assert_eq!(model.visible_task_rows(&state).len(), 1);
}
