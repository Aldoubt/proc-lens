use std::{collections::{HashMap, HashSet}, fmt};

use nvml_wrapper::{
    enum_wrappers::device::TemperatureSensor,
    enums::device::UsedGpuMemory,
    Nvml,
};

use super::{GpuDeviceSnapshot, GpuProvider, GpuSample};
use crate::process::ProcessGpuUsage;

pub struct NvmlProvider {
    nvml: Nvml,
    last_process_timestamp: HashMap<u32, u64>,
}

impl fmt::Debug for NvmlProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NvmlProvider")
            .field("tracked_devices", &self.last_process_timestamp.len())
            .finish_non_exhaustive()
    }
}

impl NvmlProvider {
    pub fn try_new() -> Option<Self> {
        Nvml::init().ok().map(|nvml| Self {
            nvml,
            last_process_timestamp: HashMap::new(),
        })
    }

    fn merge_process_memory(
        processes: &mut HashMap<i32, ProcessGpuUsage>,
        seen: &mut HashSet<(i32, u32)>,
        pid: u32,
        gpu_index: u32,
        used_gpu_memory: UsedGpuMemory,
    ) {
        let Ok(pid) = i32::try_from(pid) else {
            return;
        };
        if !seen.insert((pid, gpu_index)) {
            return;
        }
        let used_bytes = match used_gpu_memory {
            UsedGpuMemory::Used(bytes) => Some(bytes),
            UsedGpuMemory::Unavailable => None,
        };

        let entry = processes.entry(pid).or_insert(ProcessGpuUsage {
            gpu_index: Some(gpu_index),
            vram_bytes: None,
            utilization_percent: None,
        });
        if entry.gpu_index != Some(gpu_index) {
            entry.gpu_index = None;
        }
        if let Some(bytes) = used_bytes {
            entry.vram_bytes = Some(entry.vram_bytes.unwrap_or(0).saturating_add(bytes));
        }
    }

    fn merge_process_utilization(
        processes: &mut HashMap<i32, ProcessGpuUsage>,
        pid: u32,
        gpu_index: u32,
        utilization_percent: f32,
    ) {
        let Ok(pid) = i32::try_from(pid) else {
            return;
        };
        let entry = processes.entry(pid).or_insert(ProcessGpuUsage {
            gpu_index: Some(gpu_index),
            vram_bytes: None,
            utilization_percent: None,
        });
        if entry.gpu_index != Some(gpu_index) {
            entry.gpu_index = None;
            entry.utilization_percent = None;
            return;
        }
        entry.utilization_percent = Some(
            entry
                .utilization_percent
                .unwrap_or(0.0)
                .max(utilization_percent),
        );
    }
}

impl GpuProvider for NvmlProvider {
    fn sample(&mut self) -> Option<GpuSample> {
        let device_count = self.nvml.device_count().ok()?;
        if device_count == 0 {
            return None;
        }

        let mut devices = Vec::with_capacity(device_count as usize);
        let mut processes = HashMap::new();
        let mut seen_process_memory = HashSet::new();

        for index in 0..device_count {
            let Ok(device) = self.nvml.device_by_index(index) else {
                continue;
            };

            let utilization_percent = device
                .utilization_rates()
                .ok()
                .map(|usage| usage.gpu as f32);
            let memory = device.memory_info().ok();
            let temperature_c = device.temperature(TemperatureSensor::Gpu).ok();
            let power_watts = device.power_usage().ok().map(|mw| mw as f32 / 1000.0);

            devices.push(GpuDeviceSnapshot {
                index,
                name: device.name().ok(),
                utilization_percent,
                memory_used_bytes: memory.as_ref().map(|info| info.used),
                memory_total_bytes: memory.as_ref().map(|info| info.total),
                temperature_c,
                power_watts,
            });

            if let Ok(running) = device.running_compute_processes() {
                for process in running {
                    Self::merge_process_memory(
                        &mut processes,
                        &mut seen_process_memory,
                        process.pid,
                        index,
                        process.used_gpu_memory,
                    );
                }
            }
            if let Ok(running) = device.running_graphics_processes() {
                for process in running {
                    Self::merge_process_memory(
                        &mut processes,
                        &mut seen_process_memory,
                        process.pid,
                        index,
                        process.used_gpu_memory,
                    );
                }
            }

            let last_seen = self.last_process_timestamp.get(&index).copied();
            if let Ok(samples) = device.process_utilization_stats(last_seen) {
                let mut latest_per_pid = HashMap::<u32, (u64, f32)>::new();
                let mut latest_timestamp = last_seen.unwrap_or(0);
                for sample in samples {
                    latest_timestamp = latest_timestamp.max(sample.timestamp);
                    let utilization = sample.sm_util as f32;
                    let entry = latest_per_pid
                        .entry(sample.pid)
                        .or_insert((sample.timestamp, utilization));
                    if sample.timestamp >= entry.0 {
                        *entry = (sample.timestamp, utilization);
                    }
                }
                if latest_timestamp > 0 {
                    self.last_process_timestamp.insert(index, latest_timestamp);
                }
                if last_seen.is_some() {
                    for (pid, (_, utilization)) in latest_per_pid {
                        Self::merge_process_utilization(&mut processes, pid, index, utilization);
                    }
                }
            }
        }

        Some(GpuSample { devices, processes })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use nvml_wrapper::enums::device::UsedGpuMemory;

    use super::NvmlProvider;

    #[test]
    fn duplicate_context_lists_do_not_double_count_vram() {
        let mut processes = HashMap::new();
        let mut seen = HashSet::new();

        NvmlProvider::merge_process_memory(
            &mut processes,
            &mut seen,
            42,
            0,
            UsedGpuMemory::Used(512),
        );
        NvmlProvider::merge_process_memory(
            &mut processes,
            &mut seen,
            42,
            0,
            UsedGpuMemory::Used(512),
        );

        assert_eq!(processes[&42].vram_bytes, Some(512));
    }

    #[test]
    fn multi_gpu_process_does_not_expose_ambiguous_single_gpu_utilization() {
        let mut processes = HashMap::new();
        let mut seen = HashSet::new();

        NvmlProvider::merge_process_memory(
            &mut processes,
            &mut seen,
            42,
            0,
            UsedGpuMemory::Used(512),
        );
        NvmlProvider::merge_process_memory(
            &mut processes,
            &mut seen,
            42,
            1,
            UsedGpuMemory::Used(256),
        );
        NvmlProvider::merge_process_utilization(&mut processes, 42, 0, 80.0);

        assert_eq!(processes[&42].gpu_index, None);
        assert_eq!(processes[&42].vram_bytes, Some(768));
        assert_eq!(processes[&42].utilization_percent, None);
    }
}
