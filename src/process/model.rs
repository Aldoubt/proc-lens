use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessIdentity {
    pub pid: i32,
    pub start_time_ticks: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessGpuUsage {
    pub gpu_index: Option<u32>,
    pub vram_bytes: Option<u64>,
    pub utilization_percent: Option<f32>,
}

pub type LimitedEnvironment = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemorySnapshot {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

impl MemorySnapshot {
    #[must_use]
    pub fn used_bytes(self) -> u64 {
        self.total_bytes.saturating_sub(self.available_bytes)
    }

    #[must_use]
    pub fn used_percent(self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.used_bytes() as f32 / self.total_bytes as f32 * 100.0
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessSnapshot {
    pub identity: ProcessIdentity,
    pub pid: i32,
    pub ppid: i32,
    pub name: String,
    pub executable: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub command: Vec<String>,
    pub cgroup: Vec<String>,
    pub environment: LimitedEnvironment,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub gpu: Option<ProcessGpuUsage>,
}

impl ProcessSnapshot {
    #[must_use]
    pub fn command_line(&self) -> String {
        if self.command.is_empty() {
            self.name.clone()
        } else {
            self.command.join(" ")
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemSnapshot {
    pub cpu_percent: f32,
    pub memory: MemorySnapshot,
    pub load_average: [f32; 3],
    pub processes: Vec<ProcessSnapshot>,
}
