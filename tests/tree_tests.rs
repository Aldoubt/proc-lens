use std::collections::{BTreeMap, HashMap};

use proc_lens::process::model::{ProcessIdentity, ProcessSnapshot};
use proc_lens::process::tree::{parent_chain, tree_order};

fn process(pid: i32, ppid: i32) -> ProcessSnapshot {
    ProcessSnapshot {
        identity: ProcessIdentity {
            pid,
            start_time_ticks: pid as u64 * 10,
        },
        pid,
        ppid,
        name: format!("p{pid}"),
        executable: None,
        cwd: None,
        command: vec![format!("p{pid}")],
        cgroup: Vec::new(),
        environment: BTreeMap::new(),
        cpu_percent: 0.0,
        memory_bytes: 0,
        gpu: None,
    }
}

fn map(edges: &[(i32, i32)]) -> HashMap<i32, ProcessSnapshot> {
    edges
        .iter()
        .map(|(pid, ppid)| (*pid, process(*pid, *ppid)))
        .collect()
}

#[test]
fn parent_chain_stops_on_missing_parent() {
    let processes = map(&[(30, 20), (20, 10)]);
    assert_eq!(parent_chain(30, &processes, 16), vec![20, 10]);
}

#[test]
fn parent_chain_stops_on_cycle() {
    let processes = map(&[(10, 20), (20, 10)]);
    assert_eq!(parent_chain(10, &processes, 16), vec![20]);
}

#[test]
fn tree_order_places_parent_before_children() {
    let processes = map(&[(1, 0), (10, 1), (12, 10), (11, 10), (30, 1)]);
    let order = tree_order(&processes);

    assert_eq!(order, vec![(1, 0), (10, 1), (11, 2), (12, 2), (30, 1)]);
}
