use std::collections::{HashMap, HashSet};

use crate::app::{AppSnapshot, EnrichedProcess, SortMode, UiState};
use crate::process::ProcessIdentity;
use crate::provenance::{ProcessProvenance, resolve_all_provenance};
use crate::task::{TaskId, TaskSnapshot, aggregate_tasks};

pub const CPU_EMA_ALPHA: f32 = 0.35;
pub const CPU_BAND_PERCENT: f32 = 2.0;

#[derive(Debug, Clone, Copy)]
pub struct PresentationRow<'a> {
    pub process: &'a EnrichedProcess,
    pub provenance: &'a ProcessProvenance,
    pub cpu_percent: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct TaskPresentationRow<'a> {
    pub task: &'a TaskSnapshot,
    pub cpu_percent: f32,
}

#[derive(Debug, Clone)]
pub struct PresentationModel {
    snapshot: AppSnapshot,
    provenance: HashMap<ProcessIdentity, ProcessProvenance>,
    smoothed_cpu: HashMap<ProcessIdentity, f32>,
    ordered_pids: Vec<i32>,
    previous_ranks: HashMap<i32, usize>,
    tasks: Vec<TaskSnapshot>,
    smoothed_task_cpu: HashMap<TaskId, f32>,
    ordered_task_ids: Vec<TaskId>,
    task_previous_ranks: HashMap<TaskId, usize>,
}

impl PresentationModel {
    #[must_use]
    pub fn new(snapshot: AppSnapshot, state: &mut UiState) -> Self {
        let provenance = resolve_all_provenance(&snapshot.processes);
        let smoothed_cpu = snapshot
            .processes
            .iter()
            .map(|process| (process.snapshot.identity, process.snapshot.cpu_percent))
            .collect();
        let ordered_pids: Vec<i32> = snapshot
            .processes
            .iter()
            .map(|process| process.snapshot.pid)
            .collect();
        let previous_ranks = ordered_pids
            .iter()
            .enumerate()
            .map(|(rank, pid)| (*pid, rank))
            .collect();
        let tasks = aggregate_tasks(&snapshot);
        let smoothed_task_cpu = tasks
            .iter()
            .map(|task| (task.id.clone(), task.cpu_percent))
            .collect();
        let ordered_task_ids: Vec<TaskId> = tasks.iter().map(|task| task.id.clone()).collect();
        let task_previous_ranks = ordered_task_ids
            .iter()
            .cloned()
            .enumerate()
            .map(|(rank, id)| (id, rank))
            .collect();
        let mut model = Self {
            snapshot,
            provenance,
            smoothed_cpu,
            ordered_pids,
            previous_ranks,
            tasks,
            smoothed_task_cpu,
            ordered_task_ids,
            task_previous_ranks,
        };
        model.reorder(state);
        model
    }

    #[must_use]
    pub fn snapshot(&self) -> &AppSnapshot {
        &self.snapshot
    }

    pub fn integrate_snapshot(&mut self, next: AppSnapshot, state: &mut UiState, reorder: bool) {
        let previous_index = self.selected_index(state);
        let previous_task_index = self.selected_task_index(state);
        let mut next_smoothed = HashMap::with_capacity(next.processes.len());
        for process in &next.processes {
            let identity = process.snapshot.identity;
            let raw = process.snapshot.cpu_percent;
            let value = self.smoothed_cpu.get(&identity).map_or(raw, |previous| {
                CPU_EMA_ALPHA * raw + (1.0 - CPU_EMA_ALPHA) * previous
            });
            next_smoothed.insert(identity, value);
        }

        let live_pids: HashSet<i32> = next
            .processes
            .iter()
            .map(|process| process.snapshot.pid)
            .collect();
        self.ordered_pids.retain(|pid| live_pids.contains(pid));
        let mut known: HashSet<i32> = self.ordered_pids.iter().copied().collect();
        for process in &next.processes {
            if known.insert(process.snapshot.pid) {
                self.ordered_pids.push(process.snapshot.pid);
            }
        }

        let next_tasks = aggregate_tasks(&next);
        let mut next_smoothed_task = HashMap::with_capacity(next_tasks.len());
        for task in &next_tasks {
            let raw = task.cpu_percent;
            let value = self
                .smoothed_task_cpu
                .get(&task.id)
                .map_or(raw, |previous| {
                    CPU_EMA_ALPHA * raw + (1.0 - CPU_EMA_ALPHA) * previous
                });
            next_smoothed_task.insert(task.id.clone(), value);
        }
        let live_task_ids: HashSet<TaskId> =
            next_tasks.iter().map(|task| task.id.clone()).collect();
        self.ordered_task_ids
            .retain(|id| live_task_ids.contains(id));
        let mut known_tasks: HashSet<TaskId> = self.ordered_task_ids.iter().cloned().collect();
        for task in &next_tasks {
            if known_tasks.insert(task.id.clone()) {
                self.ordered_task_ids.push(task.id.clone());
            }
        }

        self.snapshot = next;
        self.provenance = resolve_all_provenance(&self.snapshot.processes);
        self.smoothed_cpu = next_smoothed;
        self.tasks = next_tasks;
        self.smoothed_task_cpu = next_smoothed_task;
        if reorder {
            self.sort_order(state);
            self.sort_task_order(state);
        }
        self.update_ranks();
        self.update_task_ranks();
        self.reconcile_selection(state, previous_index);
        self.reconcile_task_selection(state, previous_task_index);
    }

    pub fn reorder(&mut self, state: &mut UiState) {
        let previous_index = self.selected_index(state);
        let previous_task_index = self.selected_task_index(state);
        self.sort_order(state);
        self.sort_task_order(state);
        self.update_ranks();
        self.update_task_ranks();
        self.reconcile_selection(state, previous_index);
        self.reconcile_task_selection(state, previous_task_index);
    }

    pub fn reorder_preserving_index(&mut self, state: &mut UiState, previous_index: Option<usize>) {
        self.sort_order(state);
        self.update_ranks();
        self.reconcile_selection(state, previous_index);
    }

    pub fn reorder_tasks_preserving_index(
        &mut self,
        state: &mut UiState,
        previous_index: Option<usize>,
    ) {
        self.sort_task_order(state);
        self.update_task_ranks();
        self.reconcile_task_selection(state, previous_index);
    }

    #[must_use]
    pub fn visible_rows<'a>(&'a self, state: &UiState) -> Vec<PresentationRow<'a>> {
        let query = state.query.trim().to_ascii_lowercase();
        self.ordered_pids
            .iter()
            .filter_map(|pid| {
                let process = self.process_by_pid(*pid)?;
                let provenance = self.provenance.get(&process.snapshot.identity)?;
                if !state
                    .process_type_filter
                    .is_none_or(|wanted| provenance.process_type == wanted)
                {
                    return None;
                }
                if !query.is_empty() && !matches_query(process, provenance, &query) {
                    return None;
                }

                Some(PresentationRow {
                    process,
                    provenance,
                    cpu_percent: self
                        .smoothed_cpu
                        .get(&process.snapshot.identity)
                        .copied()
                        .unwrap_or(process.snapshot.cpu_percent),
                })
            })
            .collect()
    }

    #[must_use]
    pub fn visible_task_rows<'a>(&'a self, state: &UiState) -> Vec<TaskPresentationRow<'a>> {
        let query = state.query.trim().to_ascii_lowercase();
        self.ordered_task_ids
            .iter()
            .filter_map(|id| {
                let task = self.task_by_id(id)?;
                if !query.is_empty() && !self.task_matches_query(task, &query) {
                    return None;
                }
                Some(TaskPresentationRow {
                    task,
                    cpu_percent: self
                        .smoothed_task_cpu
                        .get(&task.id)
                        .copied()
                        .unwrap_or(task.cpu_percent),
                })
            })
            .collect()
    }

    #[must_use]
    pub fn ordered_pids(&self, state: &UiState) -> Vec<i32> {
        self.visible_rows(state)
            .into_iter()
            .map(|row| row.process.snapshot.pid)
            .collect()
    }

    #[must_use]
    pub fn ordered_task_ids(&self, state: &UiState) -> Vec<TaskId> {
        self.visible_task_rows(state)
            .into_iter()
            .map(|row| row.task.id.clone())
            .collect()
    }

    #[must_use]
    pub fn smoothed_cpu_for_pid(&self, pid: i32) -> Option<f32> {
        let process = self.process_by_pid(pid)?;
        self.smoothed_cpu.get(&process.snapshot.identity).copied()
    }

    #[must_use]
    pub fn smoothed_cpu_for_task(&self, task_id: &TaskId) -> Option<f32> {
        self.smoothed_task_cpu.get(task_id).copied()
    }

    #[must_use]
    pub fn selected_index(&self, state: &UiState) -> Option<usize> {
        let selected_pid = state.selected_pid?;
        self.visible_rows(state)
            .iter()
            .position(|row| row.process.snapshot.pid == selected_pid)
    }

    #[must_use]
    pub fn selected_task_index(&self, state: &UiState) -> Option<usize> {
        let selected_task_id = state.selected_task_id.as_ref()?;
        self.visible_task_rows(state)
            .iter()
            .position(|row| &row.task.id == selected_task_id)
    }

    pub fn move_selection(&self, state: &mut UiState, delta: isize) {
        let pids = self.ordered_pids(state);
        if pids.is_empty() {
            state.selected_pid = None;
            return;
        }
        let current = state
            .selected_pid
            .and_then(|pid| pids.iter().position(|candidate| *candidate == pid))
            .unwrap_or(0);
        let target = current
            .saturating_add_signed(delta)
            .min(pids.len().saturating_sub(1));
        state.selected_pid = Some(pids[target]);
    }

    pub fn move_task_selection(&self, state: &mut UiState, delta: isize) {
        let ids = self.ordered_task_ids(state);
        if ids.is_empty() {
            state.selected_task_id = None;
            return;
        }
        let current = state
            .selected_task_id
            .as_ref()
            .and_then(|id| ids.iter().position(|candidate| candidate == id))
            .unwrap_or(0);
        let target = current
            .saturating_add_signed(delta)
            .min(ids.len().saturating_sub(1));
        state.selected_task_id = Some(ids[target].clone());
    }

    pub fn move_page(&self, state: &mut UiState, page_size: usize, direction: isize) {
        let distance = page_size.max(1) as isize * direction;
        self.move_selection(state, distance);
    }

    pub fn move_task_page(&self, state: &mut UiState, page_size: usize, direction: isize) {
        let distance = page_size.max(1) as isize * direction;
        self.move_task_selection(state, distance);
    }

    pub fn select_first(&self, state: &mut UiState) {
        state.selected_pid = self.ordered_pids(state).first().copied();
    }

    pub fn select_first_task(&self, state: &mut UiState) {
        state.selected_task_id = self.ordered_task_ids(state).first().cloned();
    }

    pub fn select_last(&self, state: &mut UiState) {
        state.selected_pid = self.ordered_pids(state).last().copied();
    }

    pub fn select_last_task(&self, state: &mut UiState) {
        state.selected_task_id = self.ordered_task_ids(state).last().cloned();
    }

    fn reconcile_selection(&self, state: &mut UiState, previous_index: Option<usize>) {
        let pids = self.ordered_pids(state);
        if pids.is_empty() {
            state.selected_pid = None;
            return;
        }
        if state
            .selected_pid
            .is_some_and(|selected| pids.contains(&selected))
        {
            return;
        }
        let index = previous_index.unwrap_or(0).min(pids.len() - 1);
        state.selected_pid = Some(pids[index]);
    }

    fn reconcile_task_selection(&self, state: &mut UiState, previous_index: Option<usize>) {
        let ids = self.ordered_task_ids(state);
        if ids.is_empty() {
            state.selected_task_id = None;
            return;
        }
        if state
            .selected_task_id
            .as_ref()
            .is_some_and(|selected| ids.contains(selected))
        {
            return;
        }
        let index = previous_index.unwrap_or(0).min(ids.len() - 1);
        state.selected_task_id = Some(ids[index].clone());
    }

    fn sort_order(&mut self, state: &UiState) {
        let previous_ranks = self.previous_ranks.clone();
        let smoothed_cpu = &self.smoothed_cpu;
        let processes = &self.snapshot.processes;
        self.ordered_pids.sort_by(|left_pid, right_pid| {
            let left = processes
                .iter()
                .find(|process| process.snapshot.pid == *left_pid);
            let right = processes
                .iter()
                .find(|process| process.snapshot.pid == *right_pid);
            let (Some(left), Some(right)) = (left, right) else {
                return left_pid.cmp(right_pid);
            };

            if state.tree_mode {
                return left
                    .tree_order
                    .cmp(&right.tree_order)
                    .then_with(|| left.snapshot.pid.cmp(&right.snapshot.pid));
            }

            match state.sort {
                SortMode::Cpu => {
                    let left_cpu = smoothed_cpu
                        .get(&left.snapshot.identity)
                        .copied()
                        .unwrap_or(left.snapshot.cpu_percent);
                    let right_cpu = smoothed_cpu
                        .get(&right.snapshot.identity)
                        .copied()
                        .unwrap_or(right.snapshot.cpu_percent);
                    cpu_band(right_cpu)
                        .cmp(&cpu_band(left_cpu))
                        .then_with(|| {
                            previous_ranks
                                .get(&left.snapshot.pid)
                                .copied()
                                .unwrap_or(usize::MAX)
                                .cmp(
                                    &previous_ranks
                                        .get(&right.snapshot.pid)
                                        .copied()
                                        .unwrap_or(usize::MAX),
                                )
                        })
                        .then_with(|| left.snapshot.pid.cmp(&right.snapshot.pid))
                }
                SortMode::Memory => right
                    .snapshot
                    .memory_bytes
                    .cmp(&left.snapshot.memory_bytes)
                    .then_with(|| left.snapshot.pid.cmp(&right.snapshot.pid)),
                SortMode::Gpu => gpu_sort_value(right)
                    .total_cmp(&gpu_sort_value(left))
                    .then_with(|| gpu_vram_value(right).cmp(&gpu_vram_value(left)))
                    .then_with(|| left.snapshot.pid.cmp(&right.snapshot.pid)),
                SortMode::Pid => left.snapshot.pid.cmp(&right.snapshot.pid),
            }
        });
    }

    fn sort_task_order(&mut self, state: &UiState) {
        let previous_ranks = self.task_previous_ranks.clone();
        let smoothed_cpu = &self.smoothed_task_cpu;
        let tasks = &self.tasks;
        self.ordered_task_ids.sort_by(|left_id, right_id| {
            let left = tasks.iter().find(|task| &task.id == left_id);
            let right = tasks.iter().find(|task| &task.id == right_id);
            let (Some(left), Some(right)) = (left, right) else {
                return left_id.cmp(right_id);
            };
            match state.sort {
                SortMode::Cpu => {
                    let left_cpu = smoothed_cpu
                        .get(&left.id)
                        .copied()
                        .unwrap_or(left.cpu_percent);
                    let right_cpu = smoothed_cpu
                        .get(&right.id)
                        .copied()
                        .unwrap_or(right.cpu_percent);
                    cpu_band(right_cpu)
                        .cmp(&cpu_band(left_cpu))
                        .then_with(|| {
                            previous_ranks
                                .get(&left.id)
                                .copied()
                                .unwrap_or(usize::MAX)
                                .cmp(&previous_ranks.get(&right.id).copied().unwrap_or(usize::MAX))
                        })
                        .then_with(|| left.id.cmp(&right.id))
                }
                SortMode::Memory => right
                    .rss_bytes
                    .cmp(&left.rss_bytes)
                    .then_with(|| left.id.cmp(&right.id)),
                SortMode::Gpu => match (left.vram_bytes, right.vram_bytes) {
                    (Some(left_value), Some(right_value)) => right_value
                        .cmp(&left_value)
                        .then_with(|| left.id.cmp(&right.id)),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => left.id.cmp(&right.id),
                },
                SortMode::Pid => left
                    .label
                    .to_ascii_lowercase()
                    .cmp(&right.label.to_ascii_lowercase())
                    .then_with(|| left.id.cmp(&right.id)),
            }
        });
    }

    fn update_ranks(&mut self) {
        self.previous_ranks = self
            .ordered_pids
            .iter()
            .enumerate()
            .map(|(rank, pid)| (*pid, rank))
            .collect();
    }

    fn update_task_ranks(&mut self) {
        self.task_previous_ranks = self
            .ordered_task_ids
            .iter()
            .cloned()
            .enumerate()
            .map(|(rank, id)| (id, rank))
            .collect();
    }

    fn process_by_pid(&self, pid: i32) -> Option<&EnrichedProcess> {
        self.snapshot
            .processes
            .iter()
            .find(|process| process.snapshot.pid == pid)
    }

    fn task_by_id(&self, task_id: &TaskId) -> Option<&TaskSnapshot> {
        self.tasks.iter().find(|task| &task.id == task_id)
    }

    fn task_matches_query(&self, task: &TaskSnapshot, query: &str) -> bool {
        if task.label.to_ascii_lowercase().contains(query)
            || task.id.as_str().to_ascii_lowercase().contains(query)
            || task.kind.to_string().to_ascii_lowercase().contains(query)
        {
            return true;
        }

        task.member_ids.iter().any(|identity| {
            let Some(process) = self
                .snapshot
                .processes
                .iter()
                .find(|process| process.snapshot.identity == *identity)
            else {
                return false;
            };
            let project = self
                .provenance
                .get(identity)
                .map(|value| value.project_label.as_str())
                .unwrap_or("-");
            process.snapshot.name.to_ascii_lowercase().contains(query)
                || process
                    .snapshot
                    .command_line()
                    .to_ascii_lowercase()
                    .contains(query)
                || project.to_ascii_lowercase().contains(query)
        })
    }
}

#[must_use]
pub fn compact_command_label(process: &EnrichedProcess, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let mut parts = vec![process.snapshot.name.clone()];
    for argument in process.snapshot.command.iter().skip(1).take(2) {
        if !argument.is_empty() && !parts.iter().any(|part| part == argument) {
            parts.push(argument.clone());
        }
    }

    truncate_label(&parts.join(" "), max_chars)
}

#[must_use]
pub fn truncate_label(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    if max_chars == 0 {
        return String::new();
    }
    if max_chars == 1 {
        return "…".into();
    }

    let mut output: String = value.chars().take(max_chars - 1).collect();
    output.push('…');
    output
}

fn cpu_band(value: f32) -> i32 {
    (value / CPU_BAND_PERCENT).floor() as i32
}

fn matches_query(process: &EnrichedProcess, provenance: &ProcessProvenance, query: &str) -> bool {
    process.snapshot.name.to_ascii_lowercase().contains(query)
        || process
            .snapshot
            .command_line()
            .to_ascii_lowercase()
            .contains(query)
        || provenance
            .process_type
            .to_string()
            .to_ascii_lowercase()
            .contains(query)
        || provenance
            .project_label
            .to_ascii_lowercase()
            .contains(query)
}

fn gpu_sort_value(process: &EnrichedProcess) -> f32 {
    process
        .snapshot
        .gpu
        .as_ref()
        .and_then(|gpu| gpu.utilization_percent)
        .unwrap_or(-1.0)
}

fn gpu_vram_value(process: &EnrichedProcess) -> u64 {
    process
        .snapshot
        .gpu
        .as_ref()
        .and_then(|gpu| gpu.vram_bytes)
        .unwrap_or(0)
}
