//! Mapping between Linux processes and ROS 2 identities.

#[derive(Debug, Clone)]
pub struct RosProcessBinding {
    pub pid: u32,
    pub node_name: Option<String>,
    pub package: Option<String>,
}

pub struct RosProcessMapper;

impl RosProcessMapper {
    pub fn resolve() -> Vec<RosProcessBinding> {
        Vec::new()
    }

    pub fn find_pid(bindings: &[RosProcessBinding], pid: u32) -> Option<&RosProcessBinding> {
        bindings.iter().find(|binding| binding.pid == pid)
    }
}
