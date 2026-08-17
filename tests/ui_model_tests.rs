use std::collections::BTreeMap;

use proc_lens::app::{AppSnapshot, EnrichedProcess, SortMode, UiState};
use proc_lens::classifier::{Classification, Confidence, ProcessType};
use proc_lens::process::{MemorySnapshot, ProcessIdentity, ProcessSnapshot};
use proc_lens::ui::model::{CPU_EMA_ALPHA, PresentationModel, compact_command_label};

fn process(pid: i32, start: u64, cpu: f32, ram: u64) -> EnrichedProcess {
    EnrichedProcess {
        snapshot: ProcessSnapshot {
            identity: ProcessIdentity {
                pid,
                start_time_ticks: start,
            },
            pid,
            ppid: 1,
            name: format!("p{pid}"),
            executable: None,
            cwd: None,
            command: vec![format!("p{pid}")],
            cgroup: Vec::new(),
            environment: BTreeMap::new(),
            cpu_percent: cpu,
            memory_bytes: ram,
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

fn snapshot(processes: Vec<EnrichedProcess>) -> AppSnapshot {
    AppSnapshot {
        cpu_percent: 10.0,
        memory: MemorySnapshot {
            total_bytes: 1024,
            available_bytes: 512,
        },
        load_average: [0.1, 0.2, 0.3],
        gpu: None,
        processes,
    }
}

#[test]
fn cpu_ema_uses_alpha_point_35() {
    assert!((CPU_EMA_ALPHA - 0.35).abs() < f32::EPSILON);

    let first = snapshot(vec![process(10, 100, 10.0, 100)]);
    let mut state = UiState::default();
    let mut model = PresentationModel::new(first, &mut state);

    model.integrate_snapshot(
        snapshot(vec![process(10, 100, 20.0, 100)]),
        &mut state,
        false,
    );

    assert!((model.smoothed_cpu_for_pid(10).unwrap() - 13.5).abs() < 0.001);
}

#[test]
fn pid_reuse_resets_cpu_smoothing() {
    let first = snapshot(vec![process(10, 100, 90.0, 100)]);
    let mut state = UiState::default();
    let mut model = PresentationModel::new(first, &mut state);

    model.integrate_snapshot(
        snapshot(vec![process(10, 999, 5.0, 100)]),
        &mut state,
        false,
    );

    assert!((model.smoothed_cpu_for_pid(10).unwrap() - 5.0).abs() < 0.001);
}

#[test]
fn selected_pid_survives_reorder() {
    let first = snapshot(vec![
        process(10, 100, 20.0, 100),
        process(20, 200, 10.0, 200),
    ]);
    let mut state = UiState {
        selected_pid: Some(20),
        ..UiState::default()
    };
    let mut model = PresentationModel::new(first, &mut state);

    model.integrate_snapshot(
        snapshot(vec![
            process(10, 100, 1.0, 100),
            process(20, 200, 80.0, 200),
        ]),
        &mut state,
        true,
    );

    assert_eq!(state.selected_pid, Some(20));
    assert_eq!(model.selected_index(&state), Some(0));
}

#[test]
fn selected_pid_exit_falls_back_to_closest_visual_row() {
    let first = snapshot(vec![
        process(10, 100, 30.0, 100),
        process(20, 200, 20.0, 100),
        process(30, 300, 10.0, 100),
    ]);
    let mut state = UiState {
        selected_pid: Some(20),
        ..UiState::default()
    };
    let mut model = PresentationModel::new(first, &mut state);
    assert_eq!(model.selected_index(&state), Some(1));

    model.integrate_snapshot(
        snapshot(vec![
            process(10, 100, 30.0, 100),
            process(30, 300, 10.0, 100),
        ]),
        &mut state,
        true,
    );

    assert_eq!(state.selected_pid, Some(30));
    assert_eq!(model.selected_index(&state), Some(1));
}

#[test]
fn cpu_band_preserves_previous_order_for_small_fluctuations() {
    let first = snapshot(vec![
        process(10, 100, 11.6, 100),
        process(20, 200, 11.2, 100),
    ]);
    let mut state = UiState::default();
    let mut model = PresentationModel::new(first, &mut state);
    assert_eq!(model.ordered_pids(&state), vec![10, 20]);

    model.integrate_snapshot(
        snapshot(vec![
            process(10, 100, 10.8, 100),
            process(20, 200, 11.9, 100),
        ]),
        &mut state,
        true,
    );

    assert_eq!(model.ordered_pids(&state), vec![10, 20]);
}

#[test]
fn sort_mode_change_can_force_immediate_reorder() {
    let first = snapshot(vec![
        process(10, 100, 30.0, 100),
        process(20, 200, 10.0, 900),
    ]);
    let mut state = UiState::default();
    let mut model = PresentationModel::new(first, &mut state);
    assert_eq!(model.ordered_pids(&state), vec![10, 20]);

    state.sort = SortMode::Memory;
    model.reorder(&mut state);

    assert_eq!(model.ordered_pids(&state), vec![20, 10]);
}

#[test]
fn page_and_boundary_navigation_keep_pid_selection() {
    let first = snapshot(vec![
        process(10, 100, 40.0, 100),
        process(20, 200, 30.0, 100),
        process(30, 300, 20.0, 100),
        process(40, 400, 10.0, 100),
    ]);
    let mut state = UiState::default();
    let model = PresentationModel::new(first, &mut state);

    model.select_first(&mut state);
    assert_eq!(state.selected_pid, Some(10));

    model.move_page(&mut state, 2, 1);
    assert_eq!(state.selected_pid, Some(30));

    model.select_last(&mut state);
    assert_eq!(state.selected_pid, Some(40));

    model.move_page(&mut state, 2, -1);
    assert_eq!(state.selected_pid, Some(20));
}

#[test]
fn compact_command_hides_noisy_browser_argv() {
    let mut firefox = process(50, 500, 1.0, 100);
    firefox.snapshot.name = "firefox".into();
    firefox.snapshot.command = vec![
        "/snap/firefox/8736/usr/lib/firefox/firefox".into(),
        "-contentproc".into(),
        "-isForBrowser".into(),
        "-prefsHandle".into(),
        "0:50".into(),
        "-prefMapHandle".into(),
        "1:2345".into(),
    ];

    let label = compact_command_label(&firefox, 32);

    assert!(label.starts_with("firefox"));
    assert!(label.contains("-contentproc"));
    assert!(label.chars().count() <= 32);
    assert!(!label.contains("-prefsHandle"));
}

#[test]
fn search_filter_falls_back_to_closest_previous_visual_row() {
    let mut first = process(10, 100, 30.0, 100);
    first.snapshot.name = "keep-first".into();
    first.snapshot.command = vec!["keep-first".into()];

    let mut selected = process(20, 200, 20.0, 100);
    selected.snapshot.name = "drop-selected".into();
    selected.snapshot.command = vec!["drop-selected".into()];

    let mut last = process(30, 300, 10.0, 100);
    last.snapshot.name = "keep-last".into();
    last.snapshot.command = vec!["keep-last".into()];

    let mut state = UiState {
        selected_pid: Some(20),
        ..UiState::default()
    };
    let mut model = PresentationModel::new(snapshot(vec![first, selected, last]), &mut state);
    let previous_index = model.selected_index(&state);
    assert_eq!(previous_index, Some(1));

    state.query = "keep".into();
    model.reorder_preserving_index(&mut state, previous_index);

    assert_eq!(state.selected_pid, Some(30));
    assert_eq!(model.selected_index(&state), Some(1));
}
