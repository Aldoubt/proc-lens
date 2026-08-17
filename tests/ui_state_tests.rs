use std::collections::BTreeMap;

use proc_lens::app::{AppSnapshot, EnrichedProcess, SortMode, UiState, ViewMode};
use proc_lens::classifier::{Classification, Confidence, ProcessType};
use proc_lens::process::{MemorySnapshot, ProcessIdentity, ProcessSnapshot};

fn process(
    pid: i32,
    name: &str,
    kind: ProcessType,
    cpu: f32,
    ram: u64,
    tree_order: usize,
) -> EnrichedProcess {
    EnrichedProcess {
        snapshot: ProcessSnapshot {
            identity: ProcessIdentity {
                pid,
                start_time_ticks: pid as u64,
            },
            pid,
            ppid: 1,
            name: name.into(),
            executable: None,
            cwd: None,
            command: vec![name.into(), "--flag".into()],
            cgroup: Vec::new(),
            environment: BTreeMap::new(),
            cpu_percent: cpu,
            memory_bytes: ram,
            gpu: None,
        },
        classification: Classification {
            process_type: kind,
            confidence: Confidence::Medium,
            score: 80,
            evidence: Vec::new(),
        },
        project: None,
        parent_chain: vec![1],
        tree_depth: 1,
        tree_order,
    }
}

fn snapshot() -> AppSnapshot {
    AppSnapshot {
        cpu_percent: 20.0,
        memory: MemorySnapshot {
            total_bytes: 1000,
            available_bytes: 500,
        },
        load_average: [0.1, 0.2, 0.3],
        gpu: None,
        processes: vec![
            process(20, "clangd", ProcessType::Development, 10.0, 800, 1),
            process(10, "fast_livo", ProcessType::Ros2, 80.0, 400, 0),
        ],
    }
}

#[test]
fn ui_state_selection_is_clamped() {
    let mut state = UiState::default();
    state.move_down(2);
    state.move_down(2);
    state.move_down(2);
    assert_eq!(state.selected, 1);

    state.move_up();
    state.move_up();
    assert_eq!(state.selected, 0);
}

#[test]
fn ui_state_sort_modes_change_process_order() {
    let snapshot = snapshot();
    let mut state = UiState::default();

    assert_eq!(state.visible_processes(&snapshot)[0].snapshot.pid, 10);
    state.sort = SortMode::Memory;
    assert_eq!(state.visible_processes(&snapshot)[0].snapshot.pid, 20);
}

#[test]
fn ui_state_search_matches_type_name_and_command() {
    let snapshot = snapshot();
    let mut state = UiState::default();
    state.query = "ros2".into();
    let visible = state.visible_processes(&snapshot);
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].snapshot.pid, 10);

    state.query = "--flag".into();
    assert_eq!(state.visible_processes(&snapshot).len(), 2);
}

#[test]
fn ui_state_tree_mode_uses_tree_order() {
    let snapshot = snapshot();
    let mut state = UiState::default();
    state.sort = SortMode::Memory;
    state.tree_mode = true;

    let visible = state.visible_processes(&snapshot);
    assert_eq!(visible[0].snapshot.pid, 10);
    assert_eq!(visible[1].snapshot.pid, 20);
}

#[test]
fn ui_state_detail_can_be_opened_and_closed() {
    let mut state = UiState::default();
    assert_eq!(state.view, ViewMode::List);
    state.open_detail(1);
    assert_eq!(state.view, ViewMode::Detail);
    state.back();
    assert_eq!(state.view, ViewMode::List);
}

#[test]
fn gpu_sort_falls_back_to_vram_when_utilization_is_unavailable() {
    let mut snapshot = snapshot();
    snapshot.processes[0].snapshot.gpu = Some(proc_lens::process::ProcessGpuUsage {
        gpu_index: Some(0),
        vram_bytes: Some(2 * 1024 * 1024 * 1024),
        utilization_percent: None,
    });
    snapshot.processes[1].snapshot.gpu = Some(proc_lens::process::ProcessGpuUsage {
        gpu_index: Some(0),
        vram_bytes: Some(512 * 1024 * 1024),
        utilization_percent: None,
    });

    let mut state = UiState::default();
    state.sort = SortMode::Gpu;

    assert_eq!(state.visible_processes(&snapshot)[0].snapshot.pid, 20);
}
