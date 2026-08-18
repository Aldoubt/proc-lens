use std::ffi::CString;
use std::io;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

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

pub fn storage_for_home() -> io::Result<StorageSnapshot> {
    let target = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    storage_for_path(&target)
}

pub fn storage_for_path(path: &Path) -> io::Result<StorageSnapshot> {
    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "filesystem path contains an embedded NUL byte",
        )
    })?;
    let mut raw = MaybeUninit::<libc::statvfs>::uninit();

    // SAFETY: `c_path` is a valid NUL-terminated path and `raw` points to
    // writable memory large enough for one `statvfs` structure.
    let result = unsafe { libc::statvfs(c_path.as_ptr(), raw.as_mut_ptr()) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }

    // SAFETY: a successful `statvfs` call initializes the output structure.
    let raw = unsafe { raw.assume_init() };
    let block_size = if raw.f_frsize == 0 {
        raw.f_bsize
    } else {
        raw.f_frsize
    };

    Ok(StorageSnapshot {
        target: path.to_path_buf(),
        total_bytes: raw.f_blocks.saturating_mul(block_size),
        free_bytes: raw.f_bfree.saturating_mul(block_size),
        available_bytes: raw.f_bavail.saturating_mul(block_size),
    })
}
