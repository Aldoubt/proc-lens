use std::collections::HashMap;
use std::io;
use std::path::Path;

use crate::classifier::{Classification, ProcessType, classify, systemd_service_unit};
use crate::collector::process::{ProcCollector, ProcessIoSnapshot};
use crate::collector::storage::{StorageSnapshot, storage_for_home};
use crate::gpu::{GpuOverview, GpuProvider, GpuSample, default_provider};
use crate::process::resolver::{ProjectIdentity, resolve_ros2_process};
use crate::process::tree::{parent_chain, tree_order};
use crate::process::{MemorySnapshot, ProcessIdentity, ProcessSnapshot};
use crate::provenance::{ProcessProvenance, resolve_all_provenance, resolve_process_provenance};
use crate::task::TaskId;

#[derive(Debug, Clone, PartialEq)]
pub struct EnrichedProcess {
    pub snapshot: ProcessSnapshot,
    pub classification: Classification,
    pub project: Option<ProjectIdentity>,
    pub parent_chain: Vec<i32>,
    pub tree_depth: usize,
    pub tree_order: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppSnapshot {
    pub cpu_percent: f32,
    pub memory: MemorySnapshot,
    pub storage: Option<StorageSnapshot>,
    pub load_average: [f32; 3],
    pub gpu: Option<GpuOverview>,
    pub process_io: HashMap<ProcessIdentity, ProcessIoSnapshot>,
    pub processes: Vec<EnrichedProcess>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Cpu,
    Memory,
    Gpu,
    Pid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityMode {
    Process,
    Task,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiState {
    pub selected_pid: Option<i32>,
    pub selected_task_id: Option<TaskId>,
    pub entity_mode: EntityMode,
    pub query: String,
    pub search_active: bool,
    pub process_type_filter: Option<ProcessType>,
    pub tree_mode: bool,
    pub sort: SortMode,
    pub view: ViewMode,
    pub paused: bool,
    pub help_open: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            selected_pid: None,
            selected_task_id: None,
            entity_mode: EntityMode::Process,
            query: String::new(),
            search_active: false,
            process_type_filter: None,
            tree_mode: false,
            sort: SortMode::Cpu,
            view: ViewMode::List,
            paused: false,
            help_open: false,
        }
    }
}

impl UiState {
    pub fn open_detail(&mut self) {
        let has_selection = match self.entity_mode {
            EntityMode::Process => self.selected_pid.is_some(),
            EntityMode::Task => self.selected_task_id.is_some(),
        };
        if has_selection {
            self.view = ViewMode::Detail;
        }
    }

    pub fn back(&mut self) {
        self.view = ViewMode::List;
    }

    pub fn toggle_entity_mode(&mut self) {
        self.entity_mode = match self.entity_mode {
            EntityMode::Process => EntityMode::Task,
            EntityMode::Task => EntityMode::Process,
        };
        self.view = ViewMode::List;
        if self.entity_mode == EntityMode::Task {
            self.tree_mode = false;
        }
    }
}

pub struct Inspector {
    collector: ProcCollector,
    gpu_provider: Box<dyn GpuProvider>,
}

impl std::fmt::Debug for Inspector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inspector")
            .field("collector", &self.collector)
            .field("gpu_provider", &self.gpu_provider)
            .finish()
    }
}

impl Default for Inspector {
    fn default() -> Self {
        Self {
            collector: ProcCollector::default(),
            gpu_provider: default_provider(),
        }
    }
}

impl Inspector {
    #[must_use]
    pub fn new(collector: ProcCollector) -> Self {
        Self {
            collector,
            gpu_provider: default_provider(),
        }
    }

    pub fn with_gpu_provider(collector: ProcCollector, gpu_provider: Box<dyn GpuProvider>) -> Self {
        Self {
            collector,
            gpu_provider,
        }
    }

    pub fn refresh(&mut self) -> io::Result<AppSnapshot> {
        let system = self.collector.sample()?;
        let process_io = self.collector.latest_process_io().clone();
        let mut snapshot = enrich(system);
        snapshot.process_io = process_io;
        snapshot.storage = storage_for_home().ok();
        if let Some(gpu_sample) = self.gpu_provider.sample() {
            merge_gpu_sample(&mut snapshot, gpu_sample);
        }
        Ok(snapshot)
    }
}

fn enrich(system: crate::process::SystemSnapshot) -> AppSnapshot {
    let process_map: HashMap<i32, ProcessSnapshot> = system
        .processes
        .iter()
        .cloned()
        .map(|process| (process.pid, process))
        .collect();
    let tree_meta: HashMap<i32, (usize, usize)> = tree_order(&process_map)
        .into_iter()
        .enumerate()
        .map(|(order, (pid, depth))| (pid, (order, depth)))
        .collect();

    let processes = system
        .processes
        .into_iter()
        .map(|snapshot| {
            let chain = parent_chain(snapshot.pid, &process_map, 32);
            let ancestors: Vec<ProcessSnapshot> = chain
                .iter()
                .filter_map(|pid| process_map.get(pid).cloned())
                .collect();
            let classification = classify(&snapshot, &ancestors);
            let project = resolve_ros2_process(&snapshot);
            let (tree_order, tree_depth) = tree_meta
                .get(&snapshot.pid)
                .copied()
                .unwrap_or((usize::MAX, 0));

            EnrichedProcess {
                snapshot,
                classification,
                project,
                parent_chain: chain,
                tree_depth,
                tree_order,
            }
        })
        .collect();

    AppSnapshot {
        cpu_percent: system.cpu_percent,
        memory: system.memory,
        storage: None,
        load_average: system.load_average,
        gpu: None,
        process_io: HashMap::new(),
        processes,
    }
}

pub fn merge_gpu_sample(snapshot: &mut AppSnapshot, sample: GpuSample) {
    for process in &mut snapshot.processes {
        if let Some(usage) = sample.processes.get(&process.snapshot.pid) {
            process.snapshot.gpu = Some(usage.clone());
        }
    }
    snapshot.gpu = Some(GpuOverview {
        devices: sample.devices,
    });
}

pub fn format_snapshot(snapshot: &AppSnapshot, filter: Option<ProcessType>) -> String {
    let provenance = resolve_all_provenance(&snapshot.processes);
    let mut rows: Vec<(&EnrichedProcess, &ProcessProvenance)> = snapshot
        .processes
        .iter()
        .filter_map(|process| {
            let derived = provenance.get(&process.snapshot.identity)?;
            filter
                .is_none_or(|wanted| derived.process_type == wanted)
                .then_some((process, derived))
        })
        .collect();
    rows.sort_by(|left, right| {
        right
            .0
            .snapshot
            .cpu_percent
            .total_cmp(&left.0.snapshot.cpu_percent)
            .then_with(|| {
                right
                    .0
                    .snapshot
                    .memory_bytes
                    .cmp(&left.0.snapshot.memory_bytes)
            })
            .then_with(|| left.0.snapshot.pid.cmp(&right.0.snapshot.pid))
    });

    let mut output = String::new();
    output.push_str(&format!(
        "CPU {:5.1}%  RAM {} / {}  LOAD {:.2} {:.2} {:.2}",
        snapshot.cpu_percent,
        format_bytes(snapshot.memory.used_bytes()),
        format_bytes(snapshot.memory.total_bytes),
        snapshot.load_average[0],
        snapshot.load_average[1],
        snapshot.load_average[2],
    ));
    if let Some(storage) = &snapshot.storage {
        output.push_str(&format!(
            "  DISK {} / {} ({:.1}%, {} avail)",
            format_bytes(storage.used_bytes()),
            format_bytes(storage.total_bytes),
            storage.used_percent(),
            format_bytes(storage.available_bytes)
        ));
    }
    if let Some(device) = snapshot.gpu.as_ref().and_then(|gpu| gpu.devices.first()) {
        output.push_str(&format!(
            "  GPU{} {}",
            device.index,
            optional_percent(device.utilization_percent)
        ));
        if let (Some(used), Some(total)) = (device.memory_used_bytes, device.memory_total_bytes) {
            output.push_str(&format!(
                "  VRAM {} / {}",
                format_bytes_long(used),
                format_bytes_long(total)
            ));
        }
        if let Some(temp) = device.temperature_c {
            output.push_str(&format!("  TEMP {temp}C"));
        }
    }
    output.push_str("\n\n");
    output.push_str(&format!(
        "{:<7} {:<10} {:<24} {:>7} {:>10} {:>7} {:>10}  {}\n",
        "PID", "TYPE", "PROJECT", "CPU%", "RAM", "GPU%", "VRAM", "COMMAND"
    ));

    for (process, derived) in rows {
        output.push_str(&format!(
            "{:<7} {:<10} {:<24} {:>7.1} {:>10} {:>7} {:>10}  {}\n",
            process.snapshot.pid,
            derived.process_type,
            truncate(&derived.project_label, 24),
            process.snapshot.cpu_percent,
            format_bytes(process.snapshot.memory_bytes),
            process_gpu_label(process),
            process_vram_label(process),
            truncate(&process.snapshot.command_line(), 72),
        ));
    }

    output
}

#[must_use]
pub fn format_inspect(snapshot: &AppSnapshot, pid: i32) -> Option<String> {
    let process = snapshot
        .processes
        .iter()
        .find(|process| process.snapshot.pid == pid)?;
    let provenance = resolve_process_provenance(process, &snapshot.processes);
    let process_io = snapshot.process_io.get(&process.snapshot.identity);
    let mut output = String::new();

    output.push_str("Process Analysis\n");
    output.push_str("────────────────────────────────────────────────────────\n");
    output.push_str(&format!("PID        : {}\n", process.snapshot.pid));
    output.push_str(&format!("Name       : {}\n", process.snapshot.name));
    output.push_str(&format!(
        "Type       : {}\n",
        process.classification.process_type
    ));
    output.push_str(&format!(
        "Confidence : {}\n",
        process.classification.confidence
    ));
    output.push_str(&format!("Project    : {}\n", provenance.project_label));
    output.push_str(&format!(
        "CPU        : {:.1}%\n",
        process.snapshot.cpu_percent
    ));
    output.push_str(&format!(
        "RAM        : {}\n",
        format_bytes(process.snapshot.memory_bytes)
    ));
    output.push_str(&format!(
        "Disk read  : {}\n",
        process_io
            .map(|io| format_bytes(io.read_bytes))
            .unwrap_or_else(|| "-".into())
    ));
    output.push_str(&format!(
        "Disk write : {}\n",
        process_io
            .map(|io| format_bytes(io.write_bytes))
            .unwrap_or_else(|| "-".into())
    ));
    output.push_str(&format!(
        "Read rate  : {}\n",
        process_io
            .and_then(|io| io.read_bytes_per_second)
            .map(format_rate)
            .unwrap_or_else(|| "-".into())
    ));
    output.push_str(&format!(
        "Write rate : {}\n",
        process_io
            .and_then(|io| io.write_bytes_per_second)
            .map(format_rate)
            .unwrap_or_else(|| "-".into())
    ));
    output.push_str(&format!(
        "GPU        : {}\n",
        process
            .snapshot
            .gpu
            .as_ref()
            .and_then(|gpu| gpu.utilization_percent)
            .map(|value| format!("{value:.0}%"))
            .unwrap_or_else(|| "-".into())
    ));
    if let Some(vram) = process.snapshot.gpu.as_ref().and_then(|gpu| gpu.vram_bytes) {
        output.push_str(&format!("VRAM       : {}\n", format_bytes_long(vram)));
    }
    output.push('\n');

    output.push_str("Executable\n");
    output.push_str(&optional_path(process.snapshot.executable.as_deref()));
    output.push_str("\n\nWorking directory\n");
    output.push_str(&optional_path(process.snapshot.cwd.as_deref()));
    output.push_str("\n\nCommand\n");
    output.push_str(&process.snapshot.command_line());
    output.push_str("\n\nCGroup\n");
    if process.snapshot.cgroup.is_empty() {
        output.push('-');
    } else {
        output.push_str(&process.snapshot.cgroup.join("\n"));
    }

    output.push_str("\n\nParent chain\n");
    if process.parent_chain.is_empty() {
        output.push('-');
    } else {
        output.push_str("IDs        : ");
        output.push_str(
            &process
                .parent_chain
                .iter()
                .map(i32::to_string)
                .collect::<Vec<_>>()
                .join(" -> "),
        );
        output.push('\n');
        for (index, parent_pid) in process.parent_chain.iter().enumerate() {
            if index > 0 {
                output.push('\n');
            }
            if let Some(parent) = snapshot
                .processes
                .iter()
                .find(|candidate| candidate.snapshot.pid == *parent_pid)
            {
                output.push_str(&format!(
                    "PID {parent_pid}: {}",
                    truncate(&parent.snapshot.command_line(), 120)
                ));
            } else {
                output.push_str(&format!("PID {parent_pid}: <exited or inaccessible>"));
            }
        }
    }

    if let Some(owner_pid) = provenance.owner_pid {
        output.push_str("\n\nProvenance\n");
        output.push_str(&format!("Owner PID   : {owner_pid}\n"));
        output.push_str(&format!(
            "Owner       : {}\n",
            provenance.owner_name.as_deref().unwrap_or("-")
        ));
        output.push_str(&format!("Display type: {}\n", provenance.process_type));
        output.push_str(&format!("Project     : {}", provenance.project_label));
    }

    if let Some(project) = &process.project {
        output.push_str("\n\nROS2 identity\n");
        output.push_str(&format!(
            "Workspace  : {}\nPackage    : {}\nExecutable : {}",
            project
                .workspace
                .as_deref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "-".into()),
            project.package.as_deref().unwrap_or("-"),
            project.executable.as_deref().unwrap_or("-"),
        ));
    }

    output.push_str("\n\nClassification evidence\n");
    if process.classification.evidence.is_empty() {
        output.push_str("- no category-specific evidence");
    } else {
        for evidence in &process.classification.evidence {
            output.push_str(&format!(
                "- [{} +{}] {}\n",
                evidence.process_type, evidence.score, evidence.message
            ));
        }
        output.pop();
    }

    Some(output)
}

#[must_use]
pub fn project_label(process: &EnrichedProcess) -> String {
    if let Some(workspace) = process
        .project
        .as_ref()
        .and_then(|project| project.workspace.as_deref())
    {
        if let Some(name) = workspace.file_name().and_then(|name| name.to_str()) {
            return name.to_owned();
        }
        return workspace.display().to_string();
    }

    match process.classification.process_type {
        ProcessType::Development => {
            development_project_label(process).unwrap_or_else(|| "-".into())
        }
        ProcessType::Container => "container".into(),
        ProcessType::Systemd => systemd_unit(process).unwrap_or_else(|| "systemd".into()),
        ProcessType::Ros2 | ProcessType::Browser | ProcessType::Generic => "-".into(),
    }
}

fn development_project_label(process: &EnrichedProcess) -> Option<String> {
    let mut current = process.snapshot.cwd.as_deref()?;
    for depth in 0..=8 {
        match current.join(".git").try_exists() {
            Ok(true) => {
                return current
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned());
            }
            Ok(false) => {}
            Err(_) => return None,
        }

        if depth == 8 {
            break;
        }
        current = current.parent()?;
    }
    None
}

fn systemd_unit(process: &EnrichedProcess) -> Option<String> {
    process
        .snapshot
        .cgroup
        .iter()
        .find_map(|path| systemd_service_unit(path).map(str::to_owned))
}

fn process_gpu_label(process: &EnrichedProcess) -> String {
    process
        .snapshot
        .gpu
        .as_ref()
        .and_then(|gpu| gpu.utilization_percent)
        .map(|value| format!("{value:.0}"))
        .unwrap_or_else(|| "-".into())
}

fn process_vram_label(process: &EnrichedProcess) -> String {
    process
        .snapshot
        .gpu
        .as_ref()
        .and_then(|gpu| gpu.vram_bytes)
        .map(format_bytes)
        .unwrap_or_else(|| "-".into())
}

#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * KIB;
    const GIB: f64 = 1024.0 * MIB;
    let value = bytes as f64;

    if value >= GIB {
        format!("{:.2}G", value / GIB)
    } else if value >= MIB {
        format!("{:.1}M", value / MIB)
    } else if value >= KIB {
        format!("{:.1}K", value / KIB)
    } else {
        format!("{bytes}B")
    }
}

fn format_rate(bytes_per_second: u64) -> String {
    format!("{}/s", format_bytes(bytes_per_second))
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    let mut output: String = value.chars().take(max_chars.saturating_sub(1)).collect();
    output.push('…');
    output
}

fn optional_path(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "-".into())
}

fn optional_percent(value: Option<f32>) -> String {
    value
        .map(|value| format!("{value:.1}%"))
        .unwrap_or_else(|| "-".into())
}

fn format_bytes_long(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * KIB;
    const GIB: f64 = 1024.0 * MIB;
    let value = bytes as f64;

    if value >= GIB {
        format!("{:.2} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.2} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.2} KiB", value / KIB)
    } else {
        format!("{bytes} B")
    }
}
