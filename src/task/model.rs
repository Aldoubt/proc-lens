use std::fmt::{Display, Formatter};

use crate::process::ProcessIdentity;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(String);

impl TaskId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for TaskId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskKind {
    Ros2Launch,
    SystemdService,
    Container,
    Application,
    Process,
}

impl Display for TaskKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Ros2Launch => "ROS2",
            Self::SystemdService => "SYSTEMD",
            Self::Container => "CONTAINER",
            Self::Application => "APP",
            Self::Process => "PROCESS",
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskSnapshot {
    pub id: TaskId,
    pub kind: TaskKind,
    pub label: String,
    pub owner_pid: Option<i32>,
    pub member_ids: Vec<ProcessIdentity>,
    pub process_count: usize,
    pub cpu_percent: f32,
    pub rss_bytes: u64,
    pub disk_read_bytes: Option<u64>,
    pub disk_write_bytes: Option<u64>,
    pub read_bytes_per_second: Option<u64>,
    pub write_bytes_per_second: Option<u64>,
    pub vram_bytes: Option<u64>,
}
