use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::collector::ParseError;
use crate::collector::cpu::{
    SystemCpuTicks, cpu_percent, global_cpu_percent, parse_system_cpu_ticks,
};
use crate::collector::memory::{parse_meminfo, parse_status_rss_bytes};
use crate::process::{LimitedEnvironment, ProcessIdentity, ProcessSnapshot, SystemSnapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawProcessStat {
    pub pid: i32,
    pub name: String,
    pub state: char,
    pub ppid: i32,
    pub utime_ticks: u64,
    pub stime_ticks: u64,
    pub start_time_ticks: u64,
    pub vsize_bytes: u64,
    pub rss_pages: i64,
}

impl RawProcessStat {
    #[must_use]
    pub fn process_ticks(&self) -> u64 {
        self.utime_ticks.saturating_add(self.stime_ticks)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawProcessIo {
    pub read_bytes: u64,
    pub write_bytes: u64,
}

#[derive(Debug)]
pub struct ProcCollector {
    proc_root: PathBuf,
    previous_system: Option<SystemCpuTicks>,
    previous_process_ticks: HashMap<ProcessIdentity, u64>,
}

impl Default for ProcCollector {
    fn default() -> Self {
        Self::new("/proc")
    }
}

impl ProcCollector {
    #[must_use]
    pub fn new(proc_root: impl Into<PathBuf>) -> Self {
        Self {
            proc_root: proc_root.into(),
            previous_system: None,
            previous_process_ticks: HashMap::new(),
        }
    }

    pub fn sample(&mut self) -> io::Result<SystemSnapshot> {
        let stat_text = fs::read_to_string(self.proc_root.join("stat"))?;
        let system_ticks = parse_system_cpu_ticks(&stat_text).map_err(invalid_data)?;
        let mem_text = fs::read_to_string(self.proc_root.join("meminfo"))?;
        let memory = parse_meminfo(&mem_text).map_err(invalid_data)?;
        let load_average = read_load_average(&self.proc_root).unwrap_or([0.0; 3]);

        let mut processes = Vec::new();
        let mut current_ticks = HashMap::new();
        let previous_system = self.previous_system;

        for entry in fs::read_dir(&self.proc_root)? {
            let Ok(entry) = entry else {
                continue;
            };
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<i32>().ok())
            else {
                continue;
            };

            let process_dir = entry.path();
            let Ok(raw_stat_text) = fs::read_to_string(process_dir.join("stat")) else {
                continue;
            };
            let Ok(raw_stat) = parse_process_stat(&raw_stat_text) else {
                continue;
            };

            let identity = ProcessIdentity {
                pid,
                start_time_ticks: raw_stat.start_time_ticks,
            };
            let process_ticks = raw_stat.process_ticks();
            current_ticks.insert(identity, process_ticks);

            let process_cpu = match (
                previous_system,
                self.previous_process_ticks.get(&identity).copied(),
            ) {
                (Some(previous_system), Some(previous_process)) => cpu_percent(
                    previous_system.total_ticks,
                    system_ticks.total_ticks,
                    previous_process,
                    process_ticks,
                    system_ticks.cpu_count,
                )
                .unwrap_or(0.0),
                _ => 0.0,
            };

            let status = fs::read_to_string(process_dir.join("status")).ok();
            let memory_bytes = status
                .as_deref()
                .and_then(|text| parse_status_rss_bytes(text).ok().flatten())
                .unwrap_or(0);
            let executable = fs::read_link(process_dir.join("exe")).ok();
            let cwd = fs::read_link(process_dir.join("cwd")).ok();
            let command = fs::read(process_dir.join("cmdline"))
                .map(|bytes| parse_null_separated(&bytes))
                .unwrap_or_default();
            let cgroup = fs::read_to_string(process_dir.join("cgroup"))
                .map(|text| parse_cgroup_paths(&text))
                .unwrap_or_default();
            let environment =
                if should_read_environment(&raw_stat.name, executable.as_deref(), &command) {
                    fs::read(process_dir.join("environ"))
                        .map(|bytes| parse_selected_environment(&bytes))
                        .unwrap_or_default()
                } else {
                    BTreeMap::new()
                };

            processes.push(ProcessSnapshot {
                identity,
                pid,
                ppid: raw_stat.ppid,
                name: raw_stat.name,
                executable,
                cwd,
                command,
                cgroup,
                environment,
                cpu_percent: process_cpu,
                memory_bytes,
                gpu: None,
            });
        }

        processes.sort_unstable_by_key(|process| process.pid);
        let cpu_percent = previous_system
            .and_then(|previous| global_cpu_percent(previous, system_ticks))
            .unwrap_or(0.0);

        self.previous_system = Some(system_ticks);
        self.previous_process_ticks = current_ticks;

        Ok(SystemSnapshot {
            cpu_percent,
            memory,
            load_average,
            processes,
        })
    }
}

pub fn parse_process_stat(input: &str) -> Result<RawProcessStat, ParseError> {
    let open = input
        .find('(')
        .ok_or_else(|| ParseError::new("process stat missing opening parenthesis"))?;
    let close = input
        .rfind(')')
        .ok_or_else(|| ParseError::new("process stat missing closing parenthesis"))?;
    if close <= open {
        return Err(ParseError::new("process stat has invalid command field"));
    }

    let pid = input[..open]
        .trim()
        .parse::<i32>()
        .map_err(|_| ParseError::new("invalid pid field in process stat"))?;
    let name = input[open + 1..close].to_owned();
    let fields: Vec<&str> = input[close + 1..].split_whitespace().collect();

    // fields[0] corresponds to Linux /proc/<pid>/stat field 3 (state).
    if fields.len() < 22 {
        return Err(ParseError::new(format!(
            "process stat has too few fields after comm: {}",
            fields.len()
        )));
    }

    let state = fields[0]
        .chars()
        .next()
        .ok_or_else(|| ParseError::new("empty process state field"))?;

    Ok(RawProcessStat {
        pid,
        name,
        state,
        ppid: parse_field(fields[1], 4)?,
        utime_ticks: parse_field(fields[11], 14)?,
        stime_ticks: parse_field(fields[12], 15)?,
        start_time_ticks: parse_field(fields[19], 22)?,
        vsize_bytes: parse_field(fields[20], 23)?,
        rss_pages: parse_field(fields[21], 24)?,
    })
}

pub fn parse_process_io(input: &str) -> Result<RawProcessIo, ParseError> {
    let mut read_bytes = None;
    let mut write_bytes = None;

    for line in input.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let parsed = value
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new(format!("invalid process io field {key}")))?;
        match key.trim() {
            "read_bytes" => read_bytes = Some(parsed),
            "write_bytes" => write_bytes = Some(parsed),
            _ => {}
        }
    }

    Ok(RawProcessIo {
        read_bytes: read_bytes.ok_or_else(|| ParseError::new("process io missing read_bytes"))?,
        write_bytes: write_bytes
            .ok_or_else(|| ParseError::new("process io missing write_bytes"))?,
    })
}

#[must_use]
pub fn io_rate_bytes_per_second(previous: u64, current: u64, elapsed: Duration) -> Option<u64> {
    if current < previous || elapsed.is_zero() {
        return None;
    }

    let seconds = elapsed.as_secs_f64();
    if seconds <= 0.0 {
        return None;
    }

    Some(((current - previous) as f64 / seconds).round() as u64)
}

fn parse_field<T>(value: &str, field_number: usize) -> Result<T, ParseError>
where
    T: std::str::FromStr,
{
    value.parse::<T>().map_err(|_| {
        ParseError::new(format!(
            "invalid process stat field {field_number}: {value}"
        ))
    })
}

fn invalid_data(error: ParseError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

fn read_load_average(proc_root: &Path) -> io::Result<[f32; 3]> {
    let text = fs::read_to_string(proc_root.join("loadavg"))?;
    let mut values = text.split_whitespace();
    let one = parse_load(values.next())?;
    let five = parse_load(values.next())?;
    let fifteen = parse_load(values.next())?;
    Ok([one, five, fifteen])
}

fn parse_load(value: Option<&str>) -> io::Result<f32> {
    value
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "loadavg field missing"))?
        .parse::<f32>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid loadavg field"))
}

fn parse_null_separated(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|field| String::from_utf8_lossy(field).into_owned())
        .collect()
}

fn parse_cgroup_paths(input: &str) -> Vec<String> {
    input
        .lines()
        .filter_map(|line| line.splitn(3, ':').nth(2))
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect()
}

fn should_read_environment(name: &str, executable: Option<&Path>, command: &[String]) -> bool {
    name.contains("ros")
        || command.iter().any(|part| {
            part == "--ros-args"
                || Path::new(part)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name == "ros2")
        })
        || executable.is_some_and(|path| {
            path.components()
                .any(|part| part.as_os_str() == OsStr::new("install"))
        })
}

fn parse_selected_environment(bytes: &[u8]) -> LimitedEnvironment {
    const KEYS: [&str; 5] = [
        "ROS_VERSION",
        "ROS_DISTRO",
        "ROS_DOMAIN_ID",
        "AMENT_PREFIX_PATH",
        "COLCON_PREFIX_PATH",
    ];

    parse_null_separated(bytes)
        .into_iter()
        .filter_map(|entry| {
            let (key, value) = entry.split_once('=')?;
            KEYS.contains(&key)
                .then(|| (key.to_owned(), value.to_owned()))
        })
        .collect()
}
