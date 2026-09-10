//! ROS 2 node discovery abstraction.
//!
//! The scanner intentionally keeps transport details isolated. A future
//! implementation can connect this interface to rcl introspection or DDS
//! discovery without changing the rest of proc-lens.

#[derive(Debug, Clone)]
pub struct RosNodeInfo {
    pub name: String,
    pub namespace: String,
    pub package: Option<String>,
    pub workspace: Option<String>,
}

pub struct RosNodeScanner;

impl RosNodeScanner {
    pub fn scan() -> Vec<RosNodeInfo> {
        Vec::new()
    }

    pub fn find_by_name<'a>(nodes: &'a [RosNodeInfo], name: &str) -> Vec<&'a RosNodeInfo> {
        nodes.iter().filter(|node| node.name == name).collect()
    }
}
