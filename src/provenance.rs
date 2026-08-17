use std::collections::HashMap;

use crate::app::{EnrichedProcess, project_label};
use crate::classifier::ProcessType;
use crate::process::ProcessIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessProvenance {
    pub process_type: ProcessType,
    pub owner_pid: Option<i32>,
    pub owner_name: Option<String>,
    pub project_label: String,
}

#[must_use]
pub fn resolve_process_provenance(
    process: &EnrichedProcess,
    processes: &[EnrichedProcess],
) -> ProcessProvenance {
    let direct_type = process.classification.process_type;

    if direct_type == ProcessType::Generic {
        for parent_pid in &process.parent_chain {
            let Some(owner) = processes
                .iter()
                .find(|candidate| candidate.snapshot.pid == *parent_pid)
            else {
                continue;
            };

            match owner.classification.process_type {
                ProcessType::Browser => {
                    return ProcessProvenance {
                        process_type: ProcessType::Browser,
                        owner_pid: Some(owner.snapshot.pid),
                        owner_name: Some(owner.snapshot.name.clone()),
                        project_label: browser_family_label(owner).unwrap_or("-").to_owned(),
                    };
                }
                ProcessType::Development => {
                    return ProcessProvenance {
                        process_type: ProcessType::Development,
                        owner_pid: Some(owner.snapshot.pid),
                        owner_name: Some(owner.snapshot.name.clone()),
                        project_label: "-".into(),
                    };
                }
                ProcessType::Ros2
                | ProcessType::Container
                | ProcessType::Systemd
                | ProcessType::Generic => {}
            }
        }
    }

    let project = if direct_type == ProcessType::Browser {
        browser_family_label(process).unwrap_or("-").to_owned()
    } else {
        project_label(process)
    };

    ProcessProvenance {
        process_type: direct_type,
        owner_pid: None,
        owner_name: None,
        project_label: project,
    }
}

#[must_use]
pub fn resolve_all_provenance(
    processes: &[EnrichedProcess],
) -> HashMap<ProcessIdentity, ProcessProvenance> {
    processes
        .iter()
        .map(|process| {
            (
                process.snapshot.identity,
                resolve_process_provenance(process, processes),
            )
        })
        .collect()
}

fn browser_family_label(process: &EnrichedProcess) -> Option<&'static str> {
    browser_family_from_value(&process.snapshot.name).or_else(|| {
        process
            .snapshot
            .executable
            .as_deref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .and_then(browser_family_from_value)
    })
}

fn browser_family_from_value(value: &str) -> Option<&'static str> {
    let lower = value.to_ascii_lowercase();
    if lower.contains("firefox") {
        Some("Firefox")
    } else if lower.contains("chromium") {
        Some("Chromium")
    } else if lower.contains("brave") {
        Some("Brave")
    } else if lower.contains("chrome") {
        Some("Chrome")
    } else {
        None
    }
}
