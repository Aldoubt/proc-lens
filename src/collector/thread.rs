use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::collector::ParseError;
use crate::collector::cpu::{SystemCpuTicks, cpu_percent, parse_system_cpu_ticks};
use crate::process::ProcessSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThreadIdentity {
    pub pid: i32,
    pub process_start_time_ticks: u64,
    pub tid: i32,
    pub start_time_ticks: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThreadSnapshot {
    pub identity: ThreadIdentity,
    pub pid: i32,
    pub tid: i32,
    pub name: String,
    pub state: char,
    pub cpu_percent: Option<f32>,
    pub last_cpu: Option<u32>,
    pub priority: Option<i64>,
    pub scheduler: Option<String>,
    pub cpu_affinity: Option<String>,
    pub voluntary_context_switches: Option<u64>,
    pub involuntary_context_switches: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawThreadStat {
    pub tid: i32,
    pub name: String,
    pub state: char,
    pub utime_ticks: u64,
    pub stime_ticks: u64,
    pub start_time_ticks: u64,
    pub priority: i64,
    pub last_cpu: u32,
    pub policy: i32,
}

impl RawThreadStat {
    #[must_use]
    pub fn cpu_ticks(&self) -> u64 {
        self.utime_ticks.saturating_add(self.stime_ticks)
    }
}

#[derive(Debug)]
pub struct ThreadCollector {
    proc_root: PathBuf,
    previous_system: Option<SystemCpuTicks>,
    previous_thread_ticks: HashMap<ThreadIdentity, u64>,
}

impl Default for ThreadCollector {
    fn default() -> Self {
        Self::new("/proc")
    }
}

impl ThreadCollector {
    #[must_use]
    pub fn new(proc_root: impl Into<PathBuf>) -> Self {
        Self {
            proc_root: proc_root.into(),
            previous_system: None,
            previous_thread_ticks: HashMap::new(),
        }
    }

    pub fn sample(&mut self, processes: &[ProcessSnapshot]) -> io::Result<Vec<ThreadSnapshot>> {
        let stat_text = fs::read_to_string(self.proc_root.join("stat"))?;
        let system_ticks = parse_system_cpu_ticks(&stat_text).map_err(invalid_data)?;
        let previous_system = self.previous_system;
        let mut current_ticks = HashMap::new();
        let mut threads = Vec::new();

        for process in processes {
            let task_root = self.proc_root.join(process.pid.to_string()).join("task");
            let Ok(entries) = fs::read_dir(task_root) else {
                continue;
            };

            for entry in entries.flatten() {
                let Some(tid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|value| value.parse::<i32>().ok())
                else {
                    continue;
                };

                let thread_dir = entry.path();
                let Ok(raw_stat_text) = fs::read_to_string(thread_dir.join("stat")) else {
                    continue;
                };
                let Ok(raw_stat) = parse_thread_stat(&raw_stat_text) else {
                    continue;
                };

                let identity = ThreadIdentity {
                    pid: process.pid,
                    process_start_time_ticks: process.identity.start_time_ticks,
                    tid,
                    start_time_ticks: raw_stat.start_time_ticks,
                };
                let thread_ticks = raw_stat.cpu_ticks();
                current_ticks.insert(identity, thread_ticks);

                let cpu_percent = match (
                    previous_system,
                    self.previous_thread_ticks.get(&identity).copied(),
                ) {
                    (Some(previous_system), Some(previous_thread_ticks)) => cpu_percent(
                        previous_system.total_ticks,
                        system_ticks.total_ticks,
                        previous_thread_ticks,
                        thread_ticks,
                        system_ticks.cpu_count,
                    ),
                    _ => None,
                };

                let status = fs::read_to_string(thread_dir.join("status")).ok();
                let cpu_affinity = status
                    .as_deref()
                    .and_then(|text| status_value(text, "Cpus_allowed_list"))
                    .map(str::to_owned);
                let voluntary_context_switches = status
                    .as_deref()
                    .and_then(|text| status_u64(text, "voluntary_ctxt_switches"));
                let involuntary_context_switches = status
                    .as_deref()
                    .and_then(|text| status_u64(text, "nonvoluntary_ctxt_switches"));

                threads.push(ThreadSnapshot {
                    identity,
                    pid: process.pid,
                    tid,
                    name: raw_stat.name,
                    state: raw_stat.state,
                    cpu_percent,
                    last_cpu: Some(raw_stat.last_cpu),
                    priority: Some(raw_stat.priority),
                    scheduler: Some(scheduler_name(raw_stat.policy)),
                    cpu_affinity,
                    voluntary_context_switches,
                    involuntary_context_switches,
                });
            }
        }

        threads.sort_unstable_by_key(|thread| (thread.pid, thread.tid));
        self.previous_system = Some(system_ticks);
        self.previous_thread_ticks = current_ticks;
        Ok(threads)
    }
}

pub fn parse_thread_stat(input: &str) -> Result<RawThreadStat, ParseError> {
    let open = input
        .find('(')
        .ok_or_else(|| ParseError::new("thread stat missing opening parenthesis"))?;
    let close = input
        .rfind(')')
        .ok_or_else(|| ParseError::new("thread stat missing closing parenthesis"))?;
    if close <= open {
        return Err(ParseError::new("thread stat has invalid command field"));
    }

    let tid = input[..open]
        .trim()
        .parse::<i32>()
        .map_err(|_| ParseError::new("invalid tid field in thread stat"))?;
    let name = input[open + 1..close].to_owned();
    let fields: Vec<&str> = input[close + 1..].split_whitespace().collect();
    if fields.len() < 39 {
        return Err(ParseError::new(format!(
            "thread stat has too few fields after comm: {}",
            fields.len()
        )));
    }

    let state = fields[0]
        .chars()
        .next()
        .ok_or_else(|| ParseError::new("empty thread state field"))?;

    Ok(RawThreadStat {
        tid,
        name,
        state,
        utime_ticks: parse_field(fields[11], 14)?,
        stime_ticks: parse_field(fields[12], 15)?,
        priority: parse_field(fields[15], 18)?,
        start_time_ticks: parse_field(fields[19], 22)?,
        last_cpu: parse_field(fields[36], 39)?,
        policy: parse_field(fields[38], 41)?,
    })
}

#[must_use]
pub fn scheduler_name(policy: i32) -> String {
    match policy {
        0 => "SCHED_OTHER".to_owned(),
        1 => "SCHED_FIFO".to_owned(),
        2 => "SCHED_RR".to_owned(),
        3 => "SCHED_BATCH".to_owned(),
        5 => "SCHED_IDLE".to_owned(),
        6 => "SCHED_DEADLINE".to_owned(),
        other => format!("UNKNOWN({other})"),
    }
}

fn status_value<'a>(input: &'a str, key: &str) -> Option<&'a str> {
    input.lines().find_map(|line| {
        let (found_key, value) = line.split_once(':')?;
        (found_key == key).then_some(value.trim())
    })
}

fn status_u64(input: &str, key: &str) -> Option<u64> {
    status_value(input, key)?.parse().ok()
}

fn parse_field<T>(value: &str, field_number: usize) -> Result<T, ParseError>
where
    T: std::str::FromStr,
{
    value.parse::<T>().map_err(|_| {
        ParseError::new(format!(
            "invalid thread stat field {field_number}: {value}"
        ))
    })
}

fn invalid_data(error: ParseError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

#[cfg(test)]
mod tests {
    use super::{parse_thread_stat, scheduler_name, status_u64, status_value};

    #[test]
    fn parses_thread_stat_with_cpu_and_policy() {
        let input = "4391 (mapping worker) S 4382 0 0 0 0 0 0 0 0 0 0 100 20 0 0 20 0 1 0 123456 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 4 0 0";
        let stat = parse_thread_stat(input).expect("thread stat should parse");
        assert_eq!(stat.tid, 4391);
        assert_eq!(stat.name, "mapping worker");
        assert_eq!(stat.cpu_ticks(), 120);
        assert_eq!(stat.start_time_ticks, 123456);
        assert_eq!(stat.last_cpu, 4);
        assert_eq!(stat.priority, 20);
        assert_eq!(stat.policy, 0);
    }

    #[test]
    fn parses_optional_status_facts_without_fabricating_defaults() {
        let input = "Cpus_allowed_list:\t0-3\nvoluntary_ctxt_switches:\t42\n";
        assert_eq!(status_value(input, "Cpus_allowed_list"), Some("0-3"));
        assert_eq!(status_u64(input, "voluntary_ctxt_switches"), Some(42));
        assert_eq!(status_u64(input, "nonvoluntary_ctxt_switches"), None);
    }

    #[test]
    fn scheduler_policy_has_explicit_unknown_semantics() {
        assert_eq!(scheduler_name(0), "SCHED_OTHER");
        assert_eq!(scheduler_name(99), "UNKNOWN(99)");
    }
}
