//! ROS 2 topic topology and throughput observation models.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::node_scanner::{RosGraphError, RosNodeInfo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosTopicEndpoint {
    pub topic: String,
    pub message_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosNodeTopology {
    pub node: String,
    pub publishers: Vec<RosTopicEndpoint>,
    pub subscribers: Vec<RosTopicEndpoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosTopicInfo {
    pub name: String,
    pub message_types: Vec<String>,
    pub publishers: Vec<String>,
    pub subscribers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosTopicEdge {
    pub topic: String,
    pub publisher_node: String,
    pub subscriber_node: String,
    pub message_types: Vec<String>,
}

pub trait RosTopicTopologyProvider {
    fn discover(
        &self,
        nodes: &[RosNodeInfo],
    ) -> Result<(Vec<RosTopicInfo>, Vec<RosTopicEdge>, Duration), RosGraphError>;
}

#[derive(Debug, Clone, Copy)]
pub struct CliRosTopicTopologyProvider {
    timeout_per_node: Duration,
}

impl Default for CliRosTopicTopologyProvider {
    fn default() -> Self {
        Self {
            timeout_per_node: Duration::from_secs(2),
        }
    }
}

impl CliRosTopicTopologyProvider {
    #[must_use]
    pub fn with_timeout_per_node(timeout_per_node: Duration) -> Self {
        Self { timeout_per_node }
    }
}

impl RosTopicTopologyProvider for CliRosTopicTopologyProvider {
    fn discover(
        &self,
        nodes: &[RosNodeInfo],
    ) -> Result<(Vec<RosTopicInfo>, Vec<RosTopicEdge>, Duration), RosGraphError> {
        let started = Instant::now();
        let mut node_topologies = Vec::new();

        for node in nodes {
            match inspect_node(node, self.timeout_per_node) {
                Ok(topology) => node_topologies.push(topology),
                Err(RosGraphError::CommandNotFound | RosGraphError::NotAvailable(_)) => {
                    return Err(RosGraphError::NotAvailable(
                        "ROS 2 CLI is unavailable for topic topology discovery".to_owned(),
                    ));
                }
                Err(RosGraphError::TimedOut | RosGraphError::Failed(_)) => {
                    // A node can disappear between graph discovery and inspection. Keep the
                    // topology conservative and continue with the facts that remain observable.
                }
            }
        }

        let (topics, edges) = aggregate_topology(&node_topologies);
        Ok((topics, edges, started.elapsed()))
    }
}

fn inspect_node(node: &RosNodeInfo, timeout: Duration) -> Result<RosNodeTopology, RosGraphError> {
    let started = Instant::now();
    let mut child = Command::new("ros2")
        .arg("node")
        .arg("info")
        .arg(&node.full_name)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                RosGraphError::CommandNotFound
            } else {
                RosGraphError::NotAvailable(error.to_string())
            }
        })?;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child
                    .wait_with_output()
                    .map_err(|error| RosGraphError::Failed(error.to_string()))?;
                if !status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                    return Err(RosGraphError::Failed(if stderr.is_empty() {
                        format!("ros2 node info {} exited with {status}", node.full_name)
                    } else {
                        stderr
                    }));
                }
                return Ok(parse_node_info(&node.full_name, &output.stdout));
            }
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RosGraphError::TimedOut);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => return Err(RosGraphError::Failed(error.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndpointSection {
    None,
    Publishers,
    Subscribers,
}

fn parse_node_info(node: &str, output: &[u8]) -> RosNodeTopology {
    let mut section = EndpointSection::None;
    let mut publishers = Vec::new();
    let mut subscribers = Vec::new();

    for line in String::from_utf8_lossy(output).lines() {
        let trimmed = line.trim();
        section = match trimmed {
            "Publishers:" => EndpointSection::Publishers,
            "Subscribers:" => EndpointSection::Subscribers,
            "Service Servers:" | "Service Clients:" | "Action Servers:" | "Action Clients:" => {
                EndpointSection::None
            }
            _ => {
                if let Some(endpoint) = parse_endpoint(trimmed) {
                    match section {
                        EndpointSection::Publishers => publishers.push(endpoint),
                        EndpointSection::Subscribers => subscribers.push(endpoint),
                        EndpointSection::None => {}
                    }
                }
                section
            }
        };
    }

    RosNodeTopology {
        node: node.to_owned(),
        publishers,
        subscribers,
    }
}

fn parse_endpoint(line: &str) -> Option<RosTopicEndpoint> {
    let (topic, message_type) = line.split_once(": ")?;
    if !topic.starts_with('/') || message_type.is_empty() {
        return None;
    }
    Some(RosTopicEndpoint {
        topic: topic.to_owned(),
        message_type: message_type.to_owned(),
    })
}

#[derive(Default)]
struct TopicBuilder {
    message_types: BTreeSet<String>,
    publishers: BTreeSet<String>,
    subscribers: BTreeSet<String>,
}

fn aggregate_topology(nodes: &[RosNodeTopology]) -> (Vec<RosTopicInfo>, Vec<RosTopicEdge>) {
    let mut by_topic: BTreeMap<String, TopicBuilder> = BTreeMap::new();

    for node in nodes {
        for endpoint in &node.publishers {
            let topic = by_topic.entry(endpoint.topic.clone()).or_default();
            topic.message_types.insert(endpoint.message_type.clone());
            topic.publishers.insert(node.node.clone());
        }
        for endpoint in &node.subscribers {
            let topic = by_topic.entry(endpoint.topic.clone()).or_default();
            topic.message_types.insert(endpoint.message_type.clone());
            topic.subscribers.insert(node.node.clone());
        }
    }

    let topics: Vec<RosTopicInfo> = by_topic
        .into_iter()
        .map(|(name, topic)| RosTopicInfo {
            name,
            message_types: topic.message_types.into_iter().collect(),
            publishers: topic.publishers.into_iter().collect(),
            subscribers: topic.subscribers.into_iter().collect(),
        })
        .collect();

    let mut edges = Vec::new();
    for topic in &topics {
        for publisher_node in &topic.publishers {
            for subscriber_node in &topic.subscribers {
                edges.push(RosTopicEdge {
                    topic: topic.name.clone(),
                    publisher_node: publisher_node.clone(),
                    subscriber_node: subscriber_node.clone(),
                    message_types: topic.message_types.clone(),
                });
            }
        }
    }

    (topics, edges)
}

#[derive(Debug, Clone)]
pub struct RosTopicStat {
    pub topic: String,
    pub publisher_count: usize,
    pub subscriber_count: usize,
    pub frequency_hz: Option<f64>,
    pub bandwidth_bytes_per_sec: Option<u64>,
}

pub struct RosTopicStatsCollector;

impl RosTopicStatsCollector {
    pub fn collect() -> Vec<RosTopicStat> {
        Vec::new()
    }

    pub fn bandwidth_mb_s(stat: &RosTopicStat) -> Option<f64> {
        stat.bandwidth_bytes_per_sec
            .map(|bytes| bytes as f64 / 1024.0 / 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{RosNodeTopology, RosTopicEndpoint, aggregate_topology, parse_node_info};

    #[test]
    fn parses_publishers_and_subscribers_from_node_info() {
        let topology = parse_node_info(
            "/talker",
            b"/talker
  Subscribers:
    /parameter_events: rcl_interfaces/msg/ParameterEvent
  Publishers:
    /chatter: std_msgs/msg/String
    /rosout: rcl_interfaces/msg/Log
  Service Servers:
    /talker/describe_parameters: rcl_interfaces/srv/DescribeParameters
",
        );
        assert_eq!(topology.node, "/talker");
        assert_eq!(topology.publishers.len(), 2);
        assert_eq!(topology.publishers[0].topic, "/chatter");
        assert_eq!(topology.subscribers.len(), 1);
    }

    #[test]
    fn aggregates_topic_and_directed_node_edge() {
        let nodes = vec![
            RosNodeTopology {
                node: "/talker".to_owned(),
                publishers: vec![RosTopicEndpoint {
                    topic: "/chatter".to_owned(),
                    message_type: "std_msgs/msg/String".to_owned(),
                }],
                subscribers: Vec::new(),
            },
            RosNodeTopology {
                node: "/listener".to_owned(),
                publishers: Vec::new(),
                subscribers: vec![RosTopicEndpoint {
                    topic: "/chatter".to_owned(),
                    message_type: "std_msgs/msg/String".to_owned(),
                }],
            },
        ];

        let (topics, edges) = aggregate_topology(&nodes);
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].name, "/chatter");
        assert_eq!(topics[0].publishers, vec!["/talker"]);
        assert_eq!(topics[0].subscribers, vec!["/listener"]);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].publisher_node, "/talker");
        assert_eq!(edges[0].subscriber_node, "/listener");
    }
}
