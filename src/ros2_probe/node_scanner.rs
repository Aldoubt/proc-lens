//! ROS 2 node discovery abstraction.

use std::io;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosNodeInfo {
    pub name: String,
    pub namespace: String,
    pub full_name: String,
    pub package: Option<String>,
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RosGraphError {
    CommandNotFound,
    NotAvailable(String),
    TimedOut,
    Failed(String),
}

pub trait RosGraphProvider {
    fn discover(&self) -> Result<(Vec<RosNodeInfo>, Duration), RosGraphError>;
}

#[derive(Debug, Clone, Copy)]
pub struct CliRosGraphProvider {
    timeout: Duration,
}

impl Default for CliRosGraphProvider {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(3),
        }
    }
}

impl CliRosGraphProvider {
    #[must_use]
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl RosGraphProvider for CliRosGraphProvider {
    fn discover(&self) -> Result<(Vec<RosNodeInfo>, Duration), RosGraphError> {
        let started = Instant::now();
        let mut child = Command::new("ros2")
            .args(["node", "list"])
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
                    let elapsed = started.elapsed();
                    if !status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                        return Err(RosGraphError::Failed(if stderr.is_empty() {
                            format!("ros2 node list exited with {status}")
                        } else {
                            stderr
                        }));
                    }
                    return Ok((parse_node_list(&output.stdout), elapsed));
                }
                Ok(None) if started.elapsed() >= self.timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(RosGraphError::TimedOut);
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => return Err(RosGraphError::Failed(error.to_string())),
            }
        }
    }
}

pub struct RosNodeScanner;

impl RosNodeScanner {
    #[must_use]
    pub fn scan() -> Vec<RosNodeInfo> {
        Vec::new()
    }

    pub fn find_by_name<'a>(nodes: &'a [RosNodeInfo], name: &str) -> Vec<&'a RosNodeInfo> {
        nodes
            .iter()
            .filter(|node| node.name == name || node.full_name == name)
            .collect()
    }
}

fn parse_node_list(output: &[u8]) -> Vec<RosNodeInfo> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| {
            let full_name = line.trim();
            if full_name.is_empty() || !full_name.starts_with('/') {
                return None;
            }
            let (namespace, name) = full_name.rsplit_once('/').unwrap_or(("", full_name));
            let namespace = if namespace.is_empty() { "/" } else { namespace };
            Some(RosNodeInfo {
                name: name.to_owned(),
                namespace: namespace.to_owned(),
                full_name: full_name.to_owned(),
                package: None,
                workspace: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_node_list;

    #[test]
    fn parses_full_names_without_treating_stderr_as_nodes() {
        let nodes = parse_node_list(b"/talker\n/listener\n\n");
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].name, "talker");
        assert_eq!(nodes[1].full_name, "/listener");
    }
}
