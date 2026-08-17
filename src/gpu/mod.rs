use std::{collections::HashMap, fmt::Debug};

use crate::process::ProcessGpuUsage;

#[cfg(feature = "gpu-nvidia")]
pub mod nvml;

#[derive(Debug, Clone, PartialEq)]
pub struct GpuDeviceSnapshot {
    pub index: u32,
    pub name: Option<String>,
    pub utilization_percent: Option<f32>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub temperature_c: Option<u32>,
    pub power_watts: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GpuOverview {
    pub devices: Vec<GpuDeviceSnapshot>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GpuSample {
    pub devices: Vec<GpuDeviceSnapshot>,
    pub processes: HashMap<i32, ProcessGpuUsage>,
}

pub trait GpuProvider: Debug {
    fn sample(&mut self) -> Option<GpuSample>;
}

#[derive(Debug, Default)]
pub struct NoGpuProvider;

impl GpuProvider for NoGpuProvider {
    fn sample(&mut self) -> Option<GpuSample> {
        None
    }
}

pub fn default_provider() -> Box<dyn GpuProvider> {
    #[cfg(feature = "gpu-nvidia")]
    if let Some(provider) = nvml::NvmlProvider::try_new() {
        return Box::new(provider);
    }

    Box::new(NoGpuProvider)
}
