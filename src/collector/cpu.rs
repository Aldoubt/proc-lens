use crate::collector::ParseError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SystemCpuTicks {
    pub total_ticks: u64,
    pub idle_ticks: u64,
    pub cpu_count: usize,
}

pub fn parse_system_cpu_ticks(input: &str) -> Result<SystemCpuTicks, ParseError> {
    let mut lines = input.lines();
    let aggregate = lines
        .next()
        .ok_or_else(|| ParseError::new("/proc/stat is empty"))?;

    let mut fields = aggregate.split_whitespace();
    if fields.next() != Some("cpu") {
        return Err(ParseError::new(
            "/proc/stat first line is not aggregate cpu",
        ));
    }

    let values: Vec<u64> = fields
        .map(|field| {
            field
                .parse::<u64>()
                .map_err(|_| ParseError::new(format!("invalid cpu tick value: {field}")))
        })
        .collect::<Result<_, _>>()?;

    if values.len() < 4 {
        return Err(ParseError::new("aggregate cpu line has too few fields"));
    }

    let total_ticks = values.iter().copied().sum();
    let idle_ticks = values[3].saturating_add(values.get(4).copied().unwrap_or(0));
    let cpu_count = lines
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| {
            name.strip_prefix("cpu").is_some_and(|suffix| {
                !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit())
            })
        })
        .count();

    Ok(SystemCpuTicks {
        total_ticks,
        idle_ticks,
        cpu_count: cpu_count.max(1),
    })
}

#[must_use]
pub fn global_cpu_percent(previous: SystemCpuTicks, current: SystemCpuTicks) -> Option<f32> {
    let total_delta = current.total_ticks.checked_sub(previous.total_ticks)?;
    if total_delta == 0 {
        return None;
    }
    let idle_delta = current.idle_ticks.saturating_sub(previous.idle_ticks);
    let busy_delta = total_delta.saturating_sub(idle_delta);
    Some((busy_delta as f32 / total_delta as f32 * 100.0).clamp(0.0, 100.0))
}

#[must_use]
pub fn cpu_percent(
    previous_system_ticks: u64,
    current_system_ticks: u64,
    previous_process_ticks: u64,
    current_process_ticks: u64,
    cpu_count: usize,
) -> Option<f32> {
    let system_delta = current_system_ticks.checked_sub(previous_system_ticks)?;
    if system_delta == 0 {
        return None;
    }
    let process_delta = current_process_ticks.checked_sub(previous_process_ticks)?;
    let capacity = cpu_count.max(1) as f32;
    Some(process_delta as f32 / system_delta as f32 * capacity * 100.0)
}
