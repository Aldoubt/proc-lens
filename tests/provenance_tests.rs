use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use proc_lens::app::EnrichedProcess;
use proc_lens::classifier::{Classification, Confidence, ProcessType};
use proc_lens::process::{ProcessIdentity, ProcessSnapshot};
use proc_lens::provenance::resolve_process_provenance;

fn process(
    pid: i32,
    ppid: i32,
    name: &str,
    process_type: ProcessType,
    parent_chain: Vec<i32>,
) -> EnrichedProcess {
    EnrichedProcess {
        snapshot: ProcessSnapshot {
            identity: ProcessIdentity {
                pid,
                start_time_ticks: pid as u64 + 100,
            },
            pid,
            ppid,
            name: name.into(),
            executable: Some(PathBuf::from(format!("/usr/bin/{name}"))),
            cwd: None,
            command: vec![name.into()],
            cgroup: Vec::new(),
            environment: BTreeMap::new(),
            cpu_percent: 0.0,
            memory_bytes: 0,
            gpu: None,
        },
        classification: Classification {
            process_type,
            confidence: Confidence::Medium,
            score: 80,
            evidence: Vec::new(),
        },
        project: None,
        parent_chain,
        tree_depth: 0,
        tree_order: pid as usize,
    }
}

#[test]
fn generic_firefox_child_inherits_browser_owner() {
    let browser = process(100, 1, "firefox", ProcessType::Browser, vec![1]);
    let child = process(
        101,
        100,
        "Isolated Web Co",
        ProcessType::Generic,
        vec![100, 1],
    );
    let processes = vec![browser, child.clone()];

    let provenance = resolve_process_provenance(&child, &processes);

    assert_eq!(provenance.process_type, ProcessType::Browser);
    assert_eq!(provenance.owner_pid, Some(100));
    assert_eq!(provenance.owner_name.as_deref(), Some("firefox"));
    assert_eq!(provenance.project_label, "Firefox");
}

#[test]
fn nearest_meaningful_ancestor_wins() {
    let browser = process(100, 1, "firefox", ProcessType::Browser, vec![1]);
    let code = process(110, 100, "code", ProcessType::Development, vec![100, 1]);
    let child = process(
        111,
        110,
        "utility-process",
        ProcessType::Generic,
        vec![110, 100, 1],
    );
    let processes = vec![browser, code, child.clone()];

    let provenance = resolve_process_provenance(&child, &processes);

    assert_eq!(provenance.process_type, ProcessType::Development);
    assert_eq!(provenance.owner_pid, Some(110));
    assert_eq!(provenance.owner_name.as_deref(), Some("code"));
}

#[test]
fn direct_systemd_is_never_overwritten_by_browser_ancestor() {
    let browser = process(100, 1, "firefox", ProcessType::Browser, vec![1]);
    let service = process(
        120,
        100,
        "helper-service",
        ProcessType::Systemd,
        vec![100, 1],
    );
    let processes = vec![browser, service.clone()];

    let provenance = resolve_process_provenance(&service, &processes);

    assert_eq!(provenance.process_type, ProcessType::Systemd);
    assert_eq!(provenance.owner_pid, None);
}

#[test]
fn direct_ros2_is_never_overwritten_by_dev_ancestor() {
    let code = process(200, 1, "code", ProcessType::Development, vec![1]);
    let node = process(201, 200, "fast_livo", ProcessType::Ros2, vec![200, 1]);
    let processes = vec![code, node.clone()];

    let provenance = resolve_process_provenance(&node, &processes);

    assert_eq!(provenance.process_type, ProcessType::Ros2);
    assert_eq!(provenance.owner_pid, None);
}

#[test]
fn direct_browser_gets_normalized_family_project() {
    let browser = process(300, 1, "firefox", ProcessType::Browser, vec![1]);
    let processes = vec![browser.clone()];

    let provenance = resolve_process_provenance(&browser, &processes);

    assert_eq!(provenance.process_type, ProcessType::Browser);
    assert_eq!(provenance.owner_pid, None);
    assert_eq!(provenance.project_label, "Firefox");
}

#[test]
fn inherited_dev_project_uses_ancestor_cwd_git_root() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("proc-lens-prov-{nonce}"));
    let owner_cwd = root.join("src").join("deep");
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    fs::create_dir_all(&owner_cwd).expect("create owner cwd");

    let mut code = process(400, 1, "code", ProcessType::Development, vec![1]);
    code.snapshot.cwd = Some(owner_cwd);
    let child = process(
        401,
        400,
        "utility-process",
        ProcessType::Generic,
        vec![400, 1],
    );
    let processes = vec![code, child.clone()];

    let provenance = resolve_process_provenance(&child, &processes);

    assert_eq!(
        provenance.project_label,
        root.file_name().unwrap().to_string_lossy()
    );

    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn direct_code_without_git_falls_back_to_vscode_family() {
    let code = process(500, 1, "code", ProcessType::Development, vec![1]);
    let processes = vec![code.clone()];

    let provenance = resolve_process_provenance(&code, &processes);

    assert_eq!(provenance.project_label, "VS Code");
}

#[test]
fn generic_code_child_without_git_inherits_vscode_family() {
    let code = process(510, 1, "code", ProcessType::Development, vec![1]);
    let child = process(
        511,
        510,
        "utility-process",
        ProcessType::Generic,
        vec![510, 1],
    );
    let processes = vec![code, child.clone()];

    let provenance = resolve_process_provenance(&child, &processes);

    assert_eq!(provenance.process_type, ProcessType::Development);
    assert_eq!(provenance.project_label, "VS Code");
}

#[test]
fn direct_rust_analyzer_without_git_falls_back_to_rust_family() {
    let analyzer = process(520, 1, "rust-analyzer", ProcessType::Development, vec![1]);
    let processes = vec![analyzer.clone()];

    let provenance = resolve_process_provenance(&analyzer, &processes);

    assert_eq!(provenance.project_label, "Rust");
}

#[test]
fn real_git_project_wins_over_dev_family_fallback() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("proc-lens-dev-family-{nonce}"));
    let cwd = root.join("build");
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    fs::create_dir_all(&cwd).expect("create cwd");

    let mut clangd = process(530, 1, "clangd", ProcessType::Development, vec![1]);
    clangd.snapshot.cwd = Some(cwd);
    let processes = vec![clangd.clone()];

    let provenance = resolve_process_provenance(&clangd, &processes);

    assert_eq!(
        provenance.project_label,
        root.file_name().unwrap().to_string_lossy()
    );

    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn unknown_dev_without_git_remains_unknown() {
    let tool = process(540, 1, "custom-dev-tool", ProcessType::Development, vec![1]);
    let processes = vec![tool.clone()];

    let provenance = resolve_process_provenance(&tool, &processes);

    assert_eq!(provenance.project_label, "-");
}
