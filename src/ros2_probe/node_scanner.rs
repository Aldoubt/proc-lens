//! ROS 2 node discovery abstraction.
//!
//! The first implementation keeps collection independent from transport.
//! Future versions may use rcl introspection or ros2 daemon APIs.

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
}
