//! Conservative mapping between ROS graph facts and Linux process identities.

use std::fmt::{Display, Formatter};

use crate::process::{ProcessIdentity, ProcessSnapshot};

use super::node_scanner::RosNodeInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingConfidence {
    Exact,
    Strong,
    Heuristic,
    Unknown,
}

impl Display for MappingConfidence {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Exact => "exact",
            Self::Strong => "strong",
            Self::Heuristic => "heuristic",
            Self::Unknown => "unknown",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingSource {
    ExplicitRosArgs,
    ExecutableMatch,
    ProcessCmdline,
    LaunchAncestry,
    ComposedProcess,
    Unknown,
}

impl Display for MappingSource {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ExplicitRosArgs => "explicit_ros_args",
            Self::ExecutableMatch => "executable_match",
            Self::ProcessCmdline => "process_cmdline",
            Self::LaunchAncestry => "launch_ancestry",
            Self::ComposedProcess => "composed_process",
            Self::Unknown => "unknown",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosNodeMapping {
    pub node: RosNodeInfo,
    pub process_identity: Option<ProcessIdentity>,
    pub confidence: MappingConfidence,
    pub source: MappingSource,
}

#[derive(Debug, Clone)]
pub struct RosProcessBinding {
    pub pid: u32,
    pub node_name: Option<String>,
    pub package: Option<String>,
}

pub struct RosProcessMapper;

impl RosProcessMapper {
    pub fn resolve() -> Vec<RosProcessBinding> {
        Vec::new()
    }

    #[must_use]
    pub fn map_nodes(nodes: &[RosNodeInfo], processes: &[ProcessSnapshot]) -> Vec<RosNodeMapping> {
        nodes
            .iter()
            .cloned()
            .map(|node| map_node(node, processes))
            .collect()
    }

    pub fn find_pid(bindings: &[RosProcessBinding], pid: u32) -> Option<&RosProcessBinding> {
        bindings.iter().find(|binding| binding.pid == pid)
    }
}

fn map_node(node: RosNodeInfo, processes: &[ProcessSnapshot]) -> RosNodeMapping {
    let mut best: Option<(ProcessIdentity, MappingConfidence, MappingSource, u8)> = None;

    for process in processes {
        if let Some(remapped_name) = explicit_node_name(&process.command)
            && normalize_node_name(&remapped_name) == node.full_name
        {
            consider(
                &mut best,
                process.identity,
                MappingConfidence::Exact,
                MappingSource::ExplicitRosArgs,
                4,
            );
        }

        if process.name == node.name || executable_name(process) == Some(node.name.as_str()) {
            let source = if process.command.iter().any(|part| part == "ros2") {
                MappingSource::ProcessCmdline
            } else {
                MappingSource::ExecutableMatch
            };
            consider(
                &mut best,
                process.identity,
                MappingConfidence::Strong,
                source,
                3,
            );
        }
    }

    let (process_identity, confidence, source) = best
        .map(|(identity, confidence, source, _)| (Some(identity), confidence, source))
        .unwrap_or((None, MappingConfidence::Unknown, MappingSource::Unknown));

    RosNodeMapping {
        node,
        process_identity,
        confidence,
        source,
    }
}

fn consider(
    best: &mut Option<(ProcessIdentity, MappingConfidence, MappingSource, u8)>,
    identity: ProcessIdentity,
    confidence: MappingConfidence,
    source: MappingSource,
    rank: u8,
) {
    if best.as_ref().is_none_or(|current| rank > current.3) {
        *best = Some((identity, confidence, source, rank));
    }
}

fn executable_name(process: &ProcessSnapshot) -> Option<&str> {
    process
        .executable
        .as_deref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
}

fn explicit_node_name(command: &[String]) -> Option<String> {
    command.iter().enumerate().find_map(|(index, part)| {
        if part == "-r" || part == "--remap" {
            return command
                .get(index + 1)
                .and_then(|remap| remap.strip_prefix("__node:=").map(str::to_owned));
        }
        part.strip_prefix("__node:=").map(str::to_owned)
    })
}

fn normalize_node_name(name: &str) -> String {
    if name.starts_with('/') {
        name.to_owned()
    } else {
        format!("/{name}")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{MappingConfidence, MappingSource, RosProcessMapper};
    use crate::process::{ProcessIdentity, ProcessSnapshot};

    fn process(identity: ProcessIdentity, name: &str, command: &[&str]) -> ProcessSnapshot {
        ProcessSnapshot {
            identity,
            pid: identity.pid,
            ppid: 1,
            name: name.to_owned(),
            executable: None,
            cwd: None,
            command: command.iter().map(|part| (*part).to_owned()).collect(),
            cgroup: Vec::new(),
            environment: BTreeMap::new(),
            cpu_percent: 0.0,
            memory_bytes: 0,
            gpu: None,
        }
    }

    fn node(name: &str) -> super::RosNodeInfo {
        super::RosNodeInfo {
            name: name.trim_start_matches('/').to_owned(),
            namespace: "/".to_owned(),
            full_name: name.to_owned(),
            package: None,
            workspace: None,
        }
    }

    #[test]
    fn unknown_mapping_does_not_fabricate_pid_zero() {
        let processes = [process(
            ProcessIdentity {
                pid: 42,
                start_time_ticks: 900,
            },
            "unrelated",
            &["unrelated"],
        )];
        let mapping = &RosProcessMapper::map_nodes(&[node("/mystery")], &processes)[0];
        assert_eq!(mapping.process_identity, None);
        assert_eq!(mapping.confidence, MappingConfidence::Unknown);
        assert_eq!(mapping.source, MappingSource::Unknown);
    }

    #[test]
    fn two_nodes_can_map_to_one_process_identity() {
        let identity = ProcessIdentity {
            pid: 77,
            start_time_ticks: 1234,
        };
        let processes = [
            process(
                identity,
                "container",
                &["container", "--ros-args", "-r", "__node:=a"],
            ),
            process(
                identity,
                "container",
                &["container", "--ros-args", "-r", "__node:=b"],
            ),
        ];
        let mappings = RosProcessMapper::map_nodes(&[node("/a"), node("/b")], &processes);
        assert_eq!(mappings[0].process_identity, Some(identity));
        assert_eq!(mappings[1].process_identity, Some(identity));
    }
}
