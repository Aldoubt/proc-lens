//! ROS 2 runtime observation layer.
//!
//! This module intentionally starts with identity and topology discovery.
//! Runtime tracing (callbacks/executors) will be added in later phases.

pub mod node_scanner;
pub mod process_mapper;
pub mod topic_stats;

pub use node_scanner::{
    CliRosGraphProvider, RosGraphError, RosGraphProvider, RosNodeInfo, RosNodeScanner,
};
pub use process_mapper::{
    MappingConfidence, MappingSource, RosNodeMapping, RosProcessBinding, RosProcessMapper,
};
pub use topic_stats::{
    CliRosTopicTopologyProvider, RosNodeTopology, RosTopicEdge, RosTopicEndpoint, RosTopicInfo,
    RosTopicStat, RosTopicStatsCollector, RosTopicTopologyProvider,
};
