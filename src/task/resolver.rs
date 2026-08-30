use std::collections::HashMap;

use crate::app::EnrichedProcess;
use crate::classifier::{ProcessType, is_container_cgroup, is_ros2_launcher, systemd_service_unit};
use crate::process::ProcessIdentity;
use crate::provenance::ProcessProvenance;

use super::{TaskId, TaskKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedTask {
    pub id: TaskId,
    pub kind: TaskKind,
    pub label: String,
    pub owner_pid: Option<i32>,
}

pub(crate) fn resolve_task(
    process: &EnrichedProcess,
    processes: &[EnrichedProcess],
    provenance: &HashMap<ProcessIdentity, ProcessProvenance>,
) -> ResolvedTask {
    if let Some(owner) = nearest_ros2_launcher(process, processes) {
        return ResolvedTask {
            id: pid_task_id("ros2", owner),
            kind: TaskKind::Ros2Launch,
            label: ros2_launcher_label(owner),
            owner_pid: Some(owner.snapshot.pid),
        };
    }

    if let Some(unit) = process
        .snapshot
        .cgroup
        .iter()
        .find_map(|path| systemd_service_unit(path))
    {
        return ResolvedTask {
            id: TaskId::new(format!("systemd:{unit}")),
            kind: TaskKind::SystemdService,
            label: unit.to_owned(),
            owner_pid: None,
        };
    }

    if let Some(key) = container_key(process) {
        return ResolvedTask {
            id: TaskId::new(format!("container:{key}")),
            kind: TaskKind::Container,
            label: key.clone(),
            owner_pid: None,
        };
    }

    let derived = provenance.get(&process.snapshot.identity);
    if matches!(
        process.classification.process_type,
        ProcessType::Browser | ProcessType::Development
    ) {
        return ResolvedTask {
            id: pid_task_id("app", process),
            kind: TaskKind::Application,
            label: application_label(derived, process),
            owner_pid: Some(process.snapshot.pid),
        };
    }

    if let Some(owner_pid) = derived.and_then(|value| value.owner_pid)
        && let Some(owner) = processes
            .iter()
            .find(|candidate| candidate.snapshot.pid == owner_pid)
    {
        return ResolvedTask {
            id: pid_task_id("app", owner),
            kind: TaskKind::Application,
            label: application_label(derived, owner),
            owner_pid: Some(owner.snapshot.pid),
        };
    }

    ResolvedTask {
        id: pid_task_id("proc", process),
        kind: TaskKind::Process,
        label: process.snapshot.name.clone(),
        owner_pid: Some(process.snapshot.pid),
    }
}

fn nearest_ros2_launcher<'a>(
    process: &'a EnrichedProcess,
    processes: &'a [EnrichedProcess],
) -> Option<&'a EnrichedProcess> {
    if is_ros2_launcher(&process.snapshot) {
        return Some(process);
    }

    process.parent_chain.iter().find_map(|pid| {
        processes
            .iter()
            .find(|candidate| candidate.snapshot.pid == *pid)
            .filter(|candidate| is_ros2_launcher(&candidate.snapshot))
    })
}

fn pid_task_id(prefix: &str, process: &EnrichedProcess) -> TaskId {
    TaskId::new(format!(
        "{prefix}:{}:{}",
        process.snapshot.identity.pid, process.snapshot.identity.start_time_ticks
    ))
}

fn ros2_launcher_label(process: &EnrichedProcess) -> String {
    let command = &process.snapshot.command;
    for index in 0..command.len() {
        if !command[index].eq_ignore_ascii_case("ros2") {
            continue;
        }
        let Some(action) = command.get(index + 1) else {
            break;
        };
        if !(action.eq_ignore_ascii_case("launch") || action.eq_ignore_ascii_case("run")) {
            continue;
        }
        if let (Some(package), Some(target)) = (command.get(index + 2), command.get(index + 3)) {
            return format!("{package}/{target}");
        }
    }

    let line = process.snapshot.command_line();
    if line.chars().count() <= 64 {
        line
    } else {
        let mut label: String = line.chars().take(63).collect();
        label.push('…');
        label
    }
}

fn container_key(process: &EnrichedProcess) -> Option<String> {
    process
        .snapshot
        .cgroup
        .iter()
        .filter(|path| is_container_cgroup(path))
        .map(|path| path.trim_matches('/').to_owned())
        .min()
}

fn application_label(provenance: Option<&ProcessProvenance>, process: &EnrichedProcess) -> String {
    provenance
        .map(|value| value.project_label.as_str())
        .filter(|label| !label.is_empty() && *label != "-")
        .map(str::to_owned)
        .unwrap_or_else(|| process.snapshot.name.clone())
}
