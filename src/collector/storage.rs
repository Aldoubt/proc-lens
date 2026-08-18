use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageSnapshot {
    pub target: PathBuf,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
}

impl StorageSnapshot {
    #[must_use]
    pub fn used_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.free_bytes)
    }

    #[must_use]
    pub fn used_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.used_bytes() as f32 / self.total_bytes as f32 * 100.0
        }
    }
}
