use crate::collector::ParseError;
use crate::process::MemorySnapshot;

pub fn parse_meminfo(input: &str) -> Result<MemorySnapshot, ParseError> {
    let mut total_kib = None;
    let mut available_kib = None;
    let mut free_kib = None;
    let mut buffers_kib = 0_u64;
    let mut cached_kib = 0_u64;

    for line in input.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let value = parse_kib_value(rest)?;
        match key {
            "MemTotal" => total_kib = Some(value),
            "MemAvailable" => available_kib = Some(value),
            "MemFree" => free_kib = Some(value),
            "Buffers" => buffers_kib = value,
            "Cached" => cached_kib = value,
            _ => {}
        }
    }

    let total_kib = total_kib.ok_or_else(|| ParseError::new("MemTotal missing from /proc/meminfo"))?;
    let available_kib = available_kib.unwrap_or_else(|| {
        free_kib
            .unwrap_or(0)
            .saturating_add(buffers_kib)
            .saturating_add(cached_kib)
    });

    Ok(MemorySnapshot {
        total_bytes: total_kib.saturating_mul(1024),
        available_bytes: available_kib.min(total_kib).saturating_mul(1024),
    })
}

pub fn parse_status_rss_bytes(input: &str) -> Result<Option<u64>, ParseError> {
    for line in input.lines() {
        let Some(rest) = line.strip_prefix("VmRSS:") else {
            continue;
        };
        return parse_kib_value(rest).map(|value| Some(value.saturating_mul(1024)));
    }
    Ok(None)
}

fn parse_kib_value(input: &str) -> Result<u64, ParseError> {
    let mut fields = input.split_whitespace();
    let value = fields
        .next()
        .ok_or_else(|| ParseError::new("memory value is missing"))?
        .parse::<u64>()
        .map_err(|_| ParseError::new(format!("invalid memory value: {input}")))?;

    match fields.next() {
        Some("kB") | None => {}
        Some(unit) => return Err(ParseError::new(format!("unsupported memory unit: {unit}"))),
    }
    Ok(value)
}
