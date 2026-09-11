use std::collections::{BTreeMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::app::AppSnapshot;
use crate::classifier::ProcessType;
use crate::collector::thread::ThreadSnapshot;
use crate::process::{ProcessIdentity, ProcessSnapshot};
use crate::provenance::resolve_all_provenance;
use crate::ros2_probe::{
    RosNodeInfo, RosProcessMapper, RosTopicEdge, RosTopicInfo, RosTopicMetric,
};

pub const RUNTIME_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostRecord {
    pub arch: String,
    pub logical_cpus: usize,
}

impl HostRecord {
    #[must_use]
    pub fn current() -> Self {
        Self {
            arch: std::env::consts::ARCH.to_owned(),
            logical_cpus: std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProcessRecord {
    pub pid: i32,
    pub start_time_ticks: u64,
    pub name: String,
    pub process_type: String,
    pub project: Option<String>,
    pub cpu_percent: f32,
    pub rss_bytes: u64,
    pub thread_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThreadRecord {
    pub pid: i32,
    pub process_start_time_ticks: u64,
    pub tid: i32,
    pub start_time_ticks: u64,
    pub name: String,
    pub state: String,
    pub cpu_percent: Option<f32>,
    pub last_cpu: Option<u32>,
    pub priority: Option<i64>,
    pub scheduler: Option<String>,
    pub cpu_affinity: Option<String>,
    pub voluntary_context_switches: Option<u64>,
    pub involuntary_context_switches: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RosNodeRecord {
    pub name: String,
    pub namespace: String,
    pub full_name: String,
    pub process_identity: Option<ProcessIdentity>,
    pub mapping_confidence: String,
    pub mapping_source: String,
    pub executable: Option<String>,
    pub package: Option<String>,
}

impl RosNodeRecord {
    fn from_mapping(mapping: crate::ros2_probe::RosNodeMapping) -> Self {
        Self {
            name: mapping.node.name,
            namespace: mapping.node.namespace,
            full_name: mapping.node.full_name,
            process_identity: mapping.process_identity,
            mapping_confidence: mapping.confidence.to_string(),
            mapping_source: mapping.source.to_string(),
            executable: None,
            package: mapping.node.package,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RosTopicMetricRecord {
    pub receive_frequency_hz: Option<f64>,
    pub mean_message_bytes: Option<u64>,
    pub receive_bandwidth_bytes_per_sec: Option<u64>,
    pub sample_count: Option<u64>,
    pub observation_window_ms: u128,
    pub source: String,
    pub confidence: String,
}

impl From<&RosTopicMetric> for RosTopicMetricRecord {
    fn from(metric: &RosTopicMetric) -> Self {
        Self {
            receive_frequency_hz: metric.receive_frequency_hz,
            mean_message_bytes: metric.mean_message_bytes,
            receive_bandwidth_bytes_per_sec: metric.receive_bandwidth_bytes_per_sec,
            sample_count: metric.sample_count,
            observation_window_ms: metric.observation_window_ms,
            source: metric.source.clone(),
            confidence: metric.confidence.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RosTopicRecord {
    pub name: String,
    pub message_types: Vec<String>,
    pub publishers: Vec<String>,
    pub subscribers: Vec<String>,
    pub metrics: Option<RosTopicMetricRecord>,
}

impl RosTopicRecord {
    fn from_info(topic: &RosTopicInfo, metric: Option<&RosTopicMetric>) -> Self {
        Self {
            name: topic.name.clone(),
            message_types: topic.message_types.clone(),
            publishers: topic.publishers.clone(),
            subscribers: topic.subscribers.clone(),
            metrics: metric.map(RosTopicMetricRecord::from),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RosEdgeRecord {
    pub topic: String,
    pub publisher_node: String,
    pub subscriber_node: String,
    pub message_types: Vec<String>,
}

impl From<&RosTopicEdge> for RosEdgeRecord {
    fn from(edge: &RosTopicEdge) -> Self {
        Self {
            topic: edge.topic.clone(),
            publisher_node: edge.publisher_node.clone(),
            subscriber_node: edge.subscriber_node.clone(),
            message_types: edge.message_types.clone(),
        }
    }
}

impl From<&ThreadSnapshot> for ThreadRecord {
    fn from(thread: &ThreadSnapshot) -> Self {
        Self {
            pid: thread.pid,
            process_start_time_ticks: thread.identity.process_start_time_ticks,
            tid: thread.tid,
            start_time_ticks: thread.identity.start_time_ticks,
            name: thread.name.clone(),
            state: thread.state.to_string(),
            cpu_percent: thread.cpu_percent,
            last_cpu: thread.last_cpu,
            priority: thread.priority,
            scheduler: thread.scheduler.clone(),
            cpu_affinity: thread.cpu_affinity.clone(),
            voluntary_context_switches: thread.voluntary_context_switches,
            involuntary_context_switches: thread.involuntary_context_switches,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RuntimeSnapshot {
    pub schema_version: u32,
    pub timestamp_ns: u64,
    pub host: HostRecord,
    pub processes: Vec<ProcessRecord>,
    pub threads: Vec<ThreadRecord>,
    pub ros_nodes: Vec<RosNodeRecord>,
    pub topics: Vec<RosTopicRecord>,
    pub edges: Vec<RosEdgeRecord>,
    pub findings: Vec<serde_json::Value>,
    pub ros_graph_discovery_elapsed_ms: Option<u128>,
    pub ros_topic_topology_elapsed_ms: Option<u128>,
    pub ros_topic_metrics_elapsed_ms: Option<u128>,
}

impl RuntimeSnapshot {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_app(
        app: &AppSnapshot,
        thread_samples: &[ThreadSnapshot],
        filter: Option<ProcessType>,
        ros_nodes: &[RosNodeInfo],
        ros_topics: &[RosTopicInfo],
        ros_edges: &[RosTopicEdge],
        ros_topic_metrics: &[RosTopicMetric],
        ros_graph_discovery_elapsed_ms: Option<u128>,
        ros_topic_topology_elapsed_ms: Option<u128>,
        ros_topic_metrics_elapsed_ms: Option<u128>,
    ) -> Self {
        let provenance = resolve_all_provenance(&app.processes);
        let mut selected_processes = HashSet::new();
        let mut processes = Vec::new();

        for process in &app.processes {
            let Some(derived) = provenance.get(&process.snapshot.identity) else {
                continue;
            };
            if filter.is_some_and(|wanted| derived.process_type != wanted) {
                continue;
            }

            selected_processes.insert(process.snapshot.identity);
            let thread_count = thread_samples
                .iter()
                .filter(|thread| {
                    thread.pid == process.snapshot.pid
                        && thread.identity.process_start_time_ticks
                            == process.snapshot.identity.start_time_ticks
                })
                .count();
            let project = (derived.project_label != "-").then(|| derived.project_label.clone());

            processes.push(ProcessRecord {
                pid: process.snapshot.pid,
                start_time_ticks: process.snapshot.identity.start_time_ticks,
                name: process.snapshot.name.clone(),
                process_type: derived.process_type.to_string(),
                project,
                cpu_percent: process.snapshot.cpu_percent,
                rss_bytes: process.snapshot.memory_bytes,
                thread_count,
            });
        }

        let threads = thread_samples
            .iter()
            .filter(|thread| {
                selected_processes.contains(&ProcessIdentity {
                    pid: thread.pid,
                    start_time_ticks: thread.identity.process_start_time_ticks,
                })
            })
            .map(ThreadRecord::from)
            .collect();
        let process_snapshots: Vec<ProcessSnapshot> = app
            .processes
            .iter()
            .map(|process| process.snapshot.clone())
            .collect();
        let ros_nodes = RosProcessMapper::map_nodes(ros_nodes, &process_snapshots)
            .into_iter()
            .map(RosNodeRecord::from_mapping)
            .collect();

        let metric_by_topic: BTreeMap<&str, &RosTopicMetric> = ros_topic_metrics
            .iter()
            .map(|metric| (metric.topic.as_str(), metric))
            .collect();

        Self {
            schema_version: RUNTIME_SCHEMA_VERSION,
            timestamp_ns: now_ns(),
            host: HostRecord::current(),
            processes,
            threads,
            ros_nodes,
            topics: ros_topics
                .iter()
                .map(|topic| {
                    RosTopicRecord::from_info(topic, metric_by_topic.get(topic.name.as_str()).copied())
                })
                .collect(),
            edges: ros_edges.iter().map(RosEdgeRecord::from).collect(),
            findings: Vec::new(),
            ros_graph_discovery_elapsed_ms,
            ros_topic_topology_elapsed_ms,
            ros_topic_metrics_elapsed_ms,
        }
    }

    pub fn to_json_pretty(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::{HostRecord, RUNTIME_SCHEMA_VERSION, RuntimeSnapshot, ThreadRecord};

    #[test]
    fn runtime_json_keeps_unknown_measurements_as_null() {
        let snapshot = RuntimeSnapshot {
            schema_version: RUNTIME_SCHEMA_VERSION,
            timestamp_ns: 123,
            host: HostRecord {
                arch: "x86_64".to_owned(),
                logical_cpus: 8,
            },
            processes: Vec::new(),
            threads: vec![ThreadRecord {
                pid: 10,
                process_start_time_ticks: 100,
                tid: 11,
                start_time_ticks: 101,
                name: "worker".to_owned(),
                state: "S".to_owned(),
                cpu_percent: None,
                last_cpu: None,
                priority: None,
                scheduler: None,
                cpu_affinity: None,
                voluntary_context_switches: None,
                involuntary_context_switches: None,
            }],
            ros_nodes: Vec::new(),
            topics: Vec::new(),
            edges: Vec::new(),
            findings: Vec::new(),
            ros_graph_discovery_elapsed_ms: None,
            ros_topic_topology_elapsed_ms: None,
            ros_topic_metrics_elapsed_ms: None,
        };

        let value = serde_json::to_value(snapshot).expect("runtime snapshot should serialize");
        assert_eq!(value["schema_version"], 1);
        assert!(value["threads"][0]["cpu_percent"].is_null());
        assert!(value["threads"][0]["last_cpu"].is_null());
        assert_eq!(value["ros_nodes"], serde_json::json!([]));
        assert_eq!(value["topics"], serde_json::json!([]));
        assert_eq!(value["edges"], serde_json::json!([]));
    }

    #[test]
    fn current_host_record_is_never_empty() {
        let host = HostRecord::current();
        assert!(!host.arch.is_empty());
        assert!(host.logical_cpus >= 1);
    }
}
