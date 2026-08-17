use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::process::ProcessSnapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectIdentity {
    pub workspace: Option<PathBuf>,
    pub package: Option<String>,
    pub executable: Option<String>,
}

#[must_use]
pub fn resolve_ros2_install_path(path: &Path) -> Option<ProjectIdentity> {
    let components: Vec<_> = path.components().collect();
    let install_index = components
        .iter()
        .position(|component| component.as_os_str() == OsStr::new("install"))?;

    let package = components.get(install_index + 1)?.as_os_str().to_str()?;
    if components.get(install_index + 2)?.as_os_str() != OsStr::new("lib") {
        return None;
    }
    let lib_package = components.get(install_index + 3)?.as_os_str().to_str()?;
    if lib_package != package {
        return None;
    }
    let executable = components.last()?.as_os_str().to_str()?;
    if install_index + 4 >= components.len() {
        return None;
    }

    let mut workspace = PathBuf::new();
    for component in &components[..install_index] {
        workspace.push(component.as_os_str());
    }

    Some(ProjectIdentity {
        workspace: Some(workspace),
        package: Some(package.to_owned()),
        executable: Some(executable.to_owned()),
    })
}

#[must_use]
pub fn resolve_ros2_process(process: &ProcessSnapshot) -> Option<ProjectIdentity> {
    process
        .executable
        .as_deref()
        .and_then(resolve_ros2_install_path)
        .or_else(|| {
            process
                .command
                .iter()
                .map(Path::new)
                .find_map(resolve_ros2_install_path)
        })
}
