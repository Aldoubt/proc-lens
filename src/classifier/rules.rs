#[must_use]
pub fn is_container_cgroup(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("/docker/")
        || lower.contains("docker-")
        || lower.contains("containerd")
        || lower.contains("kubepods")
        || lower.contains("libpod-")
}

#[must_use]
pub fn systemd_service_unit(path: &str) -> Option<&str> {
    path.split('/').find(|part| {
        part.ends_with(".service")
            && !part
                .strip_prefix("user@")
                .and_then(|value| value.strip_suffix(".service"))
                .is_some_and(|uid| !uid.is_empty() && uid.bytes().all(|byte| byte.is_ascii_digit()))
    })
}

#[must_use]
pub fn known_development_executable(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "code"
            | "code-insiders"
            | "clangd"
            | "rustc"
            | "cargo"
            | "cmake"
            | "ninja"
            | "make"
            | "colcon"
            | "gcc"
            | "g++"
            | "clang"
            | "clang++"
            | "cc1"
            | "cc1plus"
            | "rust-analyzer"
            | "pyright-langserver"
            | "pylsp"
    )
}

#[must_use]
pub fn is_browser(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("firefox")
        || lower.contains("chrome")
        || lower.contains("chromium")
        || lower.contains("brave")
}
