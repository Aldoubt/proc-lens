use std::collections::BTreeMap;

use proc_lens::app::{EnrichedProcess, project_label};
use proc_lens::classifier::{Classification, Confidence, ProcessType};
use proc_lens::process::{ProcessIdentity, ProcessSnapshot};

#[test]
fn systemd_project_label_ignores_user_manager_service() {
    let process = EnrichedProcess {
        snapshot: ProcessSnapshot {
            identity: ProcessIdentity {
                pid: 2137,
                start_time_ticks: 42,
            },
            pid: 2137,
            ppid: 1,
            name: "ToDesk_Service".into(),
            executable: None,
            cwd: None,
            command: vec!["ToDesk_Service".into()],
            cgroup: vec![
                "/user.slice/user-1000.slice/user@1000.service/system.slice/todeskd.service"
                    .into(),
            ],
            environment: BTreeMap::new(),
            cpu_percent: 0.0,
            memory_bytes: 0,
            gpu: None,
        },
        classification: Classification {
            process_type: ProcessType::Systemd,
            confidence: Confidence::Medium,
            score: 80,
            evidence: Vec::new(),
        },
        project: None,
        parent_chain: vec![1],
        tree_depth: 1,
        tree_order: 0,
    };

    assert_eq!(project_label(&process), "todeskd.service");
}
