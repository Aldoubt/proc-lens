mod format;
mod model;
mod resolver;

use std::collections::{BTreeMap, HashMap};

use crate::app::{AppSnapshot, EnrichedProcess};
use crate::collector::process::ProcessIoSnapshot;
use crate::process::ProcessIdentity;
use crate::provenance::resolve_all_provenance;

pub use format::{format_task, format_tasks};
pub use model::{TaskId, TaskKind, TaskSnapshot};
use resolver::resolve_task;

#[must_use]
pub fn aggregate_tasks(snapshot: &AppSnapshot) -> Vec<TaskSnapshot> {
    let provenance = resolve_all_provenance(&snapshot.processes);
    let mut groups: BTreeMap<TaskId, (resolver::ResolvedTask, Vec<&EnrichedProcess>)> =
        BTreeMap::new();

    for process in &snapshot.processes {
        let resolved = resolve_task(process, &snapshot.processes, &provenance);
        groups
            .entry(resolved.id.clone())
            .or_insert_with(|| (resolved, Vec::new()))
            .1
            .push(process);
    }

    groups
        .into_values()
        .map(|(resolved, mut members)| {
            members.sort_by_key(|member| {
                (
                    resolved.owner_pid != Some(member.snapshot.pid),
                    member.snapshot.pid,
                )
            });

            let member_ids: Vec<ProcessIdentity> = members
                .iter()
                .map(|member| member.snapshot.identity)
                .collect();
            let cpu_percent = members
                .iter()
                .map(|member| member.snapshot.cpu_percent)
                .sum();
            let rss_bytes = members.iter().fold(0_u64, |total, member| {
                total.saturating_add(member.snapshot.memory_bytes)
            });
            let disk_read_bytes = sum_io(&members, snapshot, |io| Some(io.read_bytes));
            let disk_write_bytes = sum_io(&members, snapshot, |io| Some(io.write_bytes));
            let read_bytes_per_second = sum_io(&members, snapshot, |io| io.read_bytes_per_second);
            let write_bytes_per_second = sum_io(&members, snapshot, |io| io.write_bytes_per_second);
            let vram_bytes = sum_vram(&members);

            TaskSnapshot {
                id: resolved.id,
                kind: resolved.kind,
                label: resolved.label,
                owner_pid: resolved.owner_pid,
                process_count: member_ids.len(),
                member_ids,
                cpu_percent,
                rss_bytes,
                disk_read_bytes,
                disk_write_bytes,
                read_bytes_per_second,
                write_bytes_per_second,
                vram_bytes,
            }
        })
        .collect()
}

#[must_use]
pub fn members_for_task<'a>(
    task: &TaskSnapshot,
    snapshot: &'a AppSnapshot,
) -> Vec<&'a EnrichedProcess> {
    let processes: HashMap<ProcessIdentity, &EnrichedProcess> = snapshot
        .processes
        .iter()
        .map(|process| (process.snapshot.identity, process))
        .collect();

    task.member_ids
        .iter()
        .filter_map(|identity| processes.get(identity).copied())
        .collect()
}

fn sum_io(
    members: &[&EnrichedProcess],
    snapshot: &AppSnapshot,
    value: impl Fn(&ProcessIoSnapshot) -> Option<u64>,
) -> Option<u64> {
    let mut total = 0_u64;
    for member in members {
        let io = snapshot.process_io.get(&member.snapshot.identity)?;
        total = total.saturating_add(value(io)?);
    }
    Some(total)
}

fn sum_vram(members: &[&EnrichedProcess]) -> Option<u64> {
    let mut total = 0_u64;
    let mut seen = false;
    for member in members {
        if let Some(value) = member.snapshot.gpu.as_ref().and_then(|gpu| gpu.vram_bytes) {
            seen = true;
            total = total.saturating_add(value);
        }
    }
    seen.then_some(total)
}
