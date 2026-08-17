use std::collections::{HashMap, HashSet};

use crate::process::ProcessSnapshot;

#[must_use]
pub fn parent_chain(
    pid: i32,
    processes: &HashMap<i32, ProcessSnapshot>,
    max_depth: usize,
) -> Vec<i32> {
    let mut chain = Vec::new();
    let mut visited = HashSet::new();
    visited.insert(pid);

    let mut current = pid;
    for _ in 0..max_depth {
        let Some(process) = processes.get(&current) else {
            break;
        };
        let parent = process.ppid;
        if parent <= 0 || !visited.insert(parent) {
            break;
        }
        chain.push(parent);
        if parent == 1 {
            break;
        }
        current = parent;
    }

    chain
}

#[must_use]
pub fn tree_order(processes: &HashMap<i32, ProcessSnapshot>) -> Vec<(i32, usize)> {
    let mut children: HashMap<i32, Vec<i32>> = HashMap::new();
    let mut roots = Vec::new();

    for process in processes.values() {
        if process.ppid <= 0 || !processes.contains_key(&process.ppid) {
            roots.push(process.pid);
        } else {
            children.entry(process.ppid).or_default().push(process.pid);
        }
    }

    roots.sort_unstable();
    for values in children.values_mut() {
        values.sort_unstable();
    }

    let mut order = Vec::with_capacity(processes.len());
    let mut visited = HashSet::with_capacity(processes.len());
    for root in roots {
        visit(root, 0, &children, &mut visited, &mut order);
    }

    // A corrupted/racing snapshot can contain only cycles. Keep those processes visible.
    let mut leftovers: Vec<i32> = processes
        .keys()
        .copied()
        .filter(|pid| !visited.contains(pid))
        .collect();
    leftovers.sort_unstable();
    for pid in leftovers {
        visit(pid, 0, &children, &mut visited, &mut order);
    }

    order
}

fn visit(
    pid: i32,
    depth: usize,
    children: &HashMap<i32, Vec<i32>>,
    visited: &mut HashSet<i32>,
    output: &mut Vec<(i32, usize)>,
) {
    if !visited.insert(pid) {
        return;
    }
    output.push((pid, depth));
    if let Some(child_pids) = children.get(&pid) {
        for child in child_pids {
            visit(*child, depth.saturating_add(1), children, visited, output);
        }
    }
}
