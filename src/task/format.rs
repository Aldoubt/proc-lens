use std::collections::HashMap;

use crate::app::{AppSnapshot, EnrichedProcess, format_bytes};
use crate::process::ProcessIdentity;
use crate::provenance::resolve_all_provenance;

use super::{TaskId, TaskSnapshot, aggregate_tasks, members_for_task};

#[must_use]
pub fn format_tasks(snapshot: &AppSnapshot) -> String {
    let mut tasks = aggregate_tasks(snapshot);
    tasks.sort_by(|left, right| {
        right
            .cpu_percent
            .total_cmp(&left.cpu_percent)
            .then_with(|| right.rss_bytes.cmp(&left.rss_bytes))
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut output = String::new();
    output.push_str(&format!(
        "{:<30} {:<10} {:>5} {:>8} {:>10} {:>10} {:>10} {:>10}  {}\n",
        "TASK ID", "KIND", "PROC", "CPU%", "RSS Σ", "READ/s", "WRITE/s", "VRAM", "TASK"
    ));

    for task in tasks {
        output.push_str(&format!(
            "{:<30} {:<10} {:>5} {:>8.1} {:>10} {:>10} {:>10} {:>10}  {}\n",
            truncate(task.id.as_str(), 30),
            task.kind,
            task.process_count,
            task.cpu_percent,
            format_bytes(task.rss_bytes),
            optional_rate(task.read_bytes_per_second),
            optional_rate(task.write_bytes_per_second),
            optional_bytes(task.vram_bytes),
            truncate(&task.label, 48),
        ));
    }

    output
}

#[must_use]
pub fn format_task(snapshot: &AppSnapshot, task_id: &TaskId) -> Option<String> {
    let task = aggregate_tasks(snapshot)
        .into_iter()
        .find(|task| &task.id == task_id)?;
    Some(format_task_snapshot(snapshot, &task))
}

fn format_task_snapshot(snapshot: &AppSnapshot, task: &TaskSnapshot) -> String {
    let provenance = resolve_all_provenance(&snapshot.processes);
    let mut output = String::new();

    output.push_str("Task Analysis\n");
    output.push_str("────────────────────────────────────────────────────────\n");
    output.push_str(&format!("Task ID    : {}\n", task.id));
    output.push_str(&format!("Kind       : {}\n", task.kind));
    output.push_str(&format!("Label      : {}\n", task.label));
    output.push_str(&format!(
        "Owner PID  : {}\n",
        task.owner_pid
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| "-".into())
    ));
    output.push_str(&format!("Processes  : {}\n", task.process_count));
    output.push_str(&format!("CPU        : {:.1}%\n", task.cpu_percent));
    output.push_str(&format!("RAM RSS Σ  : {}\n", format_bytes(task.rss_bytes)));
    output.push_str(&format!(
        "Disk read  : {}\n",
        optional_bytes(task.disk_read_bytes)
    ));
    output.push_str(&format!(
        "Disk write : {}\n",
        optional_bytes(task.disk_write_bytes)
    ));
    output.push_str(&format!(
        "Read rate  : {}\n",
        optional_rate(task.read_bytes_per_second)
    ));
    output.push_str(&format!(
        "Write rate : {}\n",
        optional_rate(task.write_bytes_per_second)
    ));
    output.push_str(&format!("VRAM       : {}\n", optional_bytes(task.vram_bytes)));
    output.push_str("\nMembers\n");
    output.push_str(&format!(
        "{:<7} {:<10} {:<24} {:>7} {:>10}  {}\n",
        "PID", "TYPE", "PROJECT", "CPU%", "RAM", "COMMAND"
    ));

    let members_by_id: HashMap<ProcessIdentity, &EnrichedProcess> = members_for_task(task, snapshot)
        .into_iter()
        .map(|process| (process.snapshot.identity, process))
        .collect();

    for identity in &task.member_ids {
        let Some(process) = members_by_id.get(identity).copied() else {
            continue;
        };
        let project = provenance
            .get(identity)
            .map(|derived| derived.project_label.as_str())
            .unwrap_or("-");
        let process_type = provenance
            .get(identity)
            .map(|derived| derived.process_type.to_string())
            .unwrap_or_else(|| process.classification.process_type.to_string());
        output.push_str(&format!(
            "{:<7} {:<10} {:<24} {:>7.1} {:>10}  {}\n",
            process.snapshot.pid,
            process_type,
            truncate(project, 24),
            process.snapshot.cpu_percent,
            format_bytes(process.snapshot.memory_bytes),
            truncate(&process.snapshot.command_line(), 72),
        ));
    }

    output
}

fn optional_rate(value: Option<u64>) -> String {
    value
        .map(|bytes| format!("{}/s", format_bytes(bytes)))
        .unwrap_or_else(|| "-".into())
}

fn optional_bytes(value: Option<u64>) -> String {
    value.map(format_bytes).unwrap_or_else(|| "-".into())
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    let mut output: String = value.chars().take(max_chars.saturating_sub(1)).collect();
    output.push('…');
    output
}
