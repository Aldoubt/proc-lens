mod rules;

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use crate::process::ProcessSnapshot;
use crate::process::resolver::resolve_ros2_process;

pub use rules::{is_container_cgroup, known_development_executable, systemd_service_unit};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProcessType {
    Ros2,
    Container,
    Systemd,
    Development,
    Browser,
    Generic,
}

impl ProcessType {
    pub const ALL: [Self; 6] = [
        Self::Ros2,
        Self::Container,
        Self::Systemd,
        Self::Development,
        Self::Browser,
        Self::Generic,
    ];
}

impl Display for ProcessType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Ros2 => "ROS2",
            Self::Container => "CONTAINER",
            Self::Systemd => "SYSTEMD",
            Self::Development => "DEV",
            Self::Browser => "BROWSER",
            Self::Generic => "PROCESS",
        })
    }
}

impl FromStr for ProcessType {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ros" | "ros2" => Ok(Self::Ros2),
            "container" | "docker" => Ok(Self::Container),
            "systemd" | "service" => Ok(Self::Systemd),
            "dev" | "development" => Ok(Self::Development),
            "browser" => Ok(Self::Browser),
            "generic" | "process" => Ok(Self::Generic),
            other => Err(format!("unknown process type: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    Low,
    Medium,
    High,
}

impl Display for Confidence {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub process_type: ProcessType,
    pub score: i32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classification {
    pub process_type: ProcessType,
    pub confidence: Confidence,
    pub score: i32,
    pub evidence: Vec<Evidence>,
}

#[must_use]
pub fn classify(process: &ProcessSnapshot, ancestors: &[ProcessSnapshot]) -> Classification {
    let mut evidence = Vec::new();

    if resolve_ros2_process(process).is_some() {
        push(
            &mut evidence,
            ProcessType::Ros2,
            80,
            "executable matches install/<package>/lib/<package>/<executable>",
        );
    }

    if process.command.iter().any(|part| part == "--ros-args") {
        push(
            &mut evidence,
            ProcessType::Ros2,
            70,
            "command contains --ros-args",
        );
    }

    if is_ros2_launcher(process) {
        push(
            &mut evidence,
            ProcessType::Ros2,
            60,
            "command contains ros2 launch/run",
        );
    }

    if ancestors.iter().any(is_ros2_launcher) {
        push(
            &mut evidence,
            ProcessType::Ros2,
            50,
            "ancestor command contains ros2 launch/run",
        );
    }

    if process
        .environment
        .get("ROS_VERSION")
        .is_some_and(|v| v == "2")
    {
        push(
            &mut evidence,
            ProcessType::Ros2,
            40,
            "environment contains ROS_VERSION=2",
        );
    }
    if process.environment.contains_key("AMENT_PREFIX_PATH") {
        push(
            &mut evidence,
            ProcessType::Ros2,
            20,
            "environment contains AMENT_PREFIX_PATH",
        );
    }

    if process
        .cgroup
        .iter()
        .any(|path| rules::is_container_cgroup(path))
    {
        push(
            &mut evidence,
            ProcessType::Container,
            90,
            "cgroup contains docker/container runtime marker",
        );
    }

    if process
        .cgroup
        .iter()
        .any(|path| rules::systemd_service_unit(path).is_some())
    {
        push(
            &mut evidence,
            ProcessType::Systemd,
            80,
            "cgroup contains a concrete systemd .service unit",
        );
    }

    if rules::known_development_executable(&process.name)
        || process
            .executable
            .as_deref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .is_some_and(rules::known_development_executable)
    {
        push(
            &mut evidence,
            ProcessType::Development,
            70,
            "executable matches a known development/build tool",
        );
    }

    if rules::is_browser(&process.name) {
        push(
            &mut evidence,
            ProcessType::Browser,
            80,
            "process name matches a known browser",
        );
    }

    let strong_ros2 = evidence
        .iter()
        .any(|item| item.process_type == ProcessType::Ros2 && item.score >= 50);

    let mut scores: BTreeMap<ProcessType, i32> = BTreeMap::new();
    for item in &evidence {
        *scores.entry(item.process_type).or_default() += item.score;
    }

    let process_type = winning_type(&scores, strong_ros2);
    let score = scores.get(&process_type).copied().unwrap_or(0);
    let confidence = match score {
        100.. => Confidence::High,
        60..=99 => Confidence::Medium,
        _ => Confidence::Low,
    };

    evidence.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.message.cmp(&right.message))
    });

    Classification {
        process_type,
        confidence,
        score,
        evidence,
    }
}

fn push(evidence: &mut Vec<Evidence>, process_type: ProcessType, score: i32, message: &str) {
    evidence.push(Evidence {
        process_type,
        score,
        message: message.to_owned(),
    });
}

#[must_use]
pub fn is_ros2_launcher(process: &ProcessSnapshot) -> bool {
    let joined = process.command_line().to_ascii_lowercase();
    joined.contains("ros2 launch") || joined.contains("ros2 run")
}

fn winning_type(scores: &BTreeMap<ProcessType, i32>, strong_ros2: bool) -> ProcessType {
    if strong_ros2 {
        return ProcessType::Ros2;
    }

    const PRIORITY: [ProcessType; 5] = [
        ProcessType::Ros2,
        ProcessType::Container,
        ProcessType::Systemd,
        ProcessType::Development,
        ProcessType::Browser,
    ];

    let mut best = ProcessType::Generic;
    let mut best_score = 0;
    for process_type in PRIORITY {
        let score = scores.get(&process_type).copied().unwrap_or(0);
        if score > best_score {
            best = process_type;
            best_score = score;
        }
    }
    best
}
