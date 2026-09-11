//! Low-overhead ROS 2 topic metric sampling.
//!
//! Metrics are observed from temporary ROS 2 CLI subscriptions. They describe
//! receive-side behavior during a bounded sampling window, not publisher-side
//! scheduling guarantees.

use std::fmt::{Display, Formatter};
use std::io;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::topic_stats::RosTopicInfo;

const DEFAULT_SAMPLE_WINDOW: Duration = Duration::from_secs(2);
const DEFAULT_MAX_TOPICS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricConfidence {
    Observed,
    Partial,
    Unknown,
}

impl Display for MetricConfidence {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Observed => "observed",
            Self::Partial => "partial",
            Self::Unknown => "unknown",
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RosTopicMetric {
    pub topic: String,
    pub receive_frequency_hz: Option<f64>,
    pub mean_message_bytes: Option<u64>,
    pub receive_bandwidth_bytes_per_sec: Option<u64>,
    pub sample_count: Option<u64>,
    pub observation_window_ms: u128,
    pub source: String,
    pub confidence: MetricConfidence,
}

#[derive(Debug, Clone, Copy)]
pub struct CliRosTopicMetricsCollector {
    sample_window: Duration,
    max_topics: usize,
}

impl Default for CliRosTopicMetricsCollector {
    fn default() -> Self {
        Self {
            sample_window: DEFAULT_SAMPLE_WINDOW,
            max_topics: DEFAULT_MAX_TOPICS,
        }
    }
}

impl CliRosTopicMetricsCollector {
    #[must_use]
    pub fn with_sample_window(mut self, sample_window: Duration) -> Self {
        self.sample_window = sample_window;
        self
    }

    #[must_use]
    pub fn with_max_topics(mut self, max_topics: usize) -> Self {
        self.max_topics = max_topics;
        self
    }

    #[must_use]
    pub fn collect(&self, topics: &[RosTopicInfo]) -> (Vec<RosTopicMetric>, Duration) {
        let started = Instant::now();
        let selected = select_metric_topics(topics, self.max_topics);
        let mut handles = Vec::with_capacity(selected.len());

        for topic in selected {
            let topic_name = topic.name.clone();
            let sample_window = self.sample_window;
            handles.push(thread::spawn(move || observe_topic(topic_name, sample_window)));
        }

        let metrics = handles
            .into_iter()
            .filter_map(|handle| handle.join().ok())
            .collect();

        (metrics, started.elapsed())
    }
}

fn select_metric_topics(topics: &[RosTopicInfo], max_topics: usize) -> Vec<&RosTopicInfo> {
    topics
        .iter()
        .filter(|topic| {
            !topic.publishers.is_empty()
                && !topic.subscribers.is_empty()
                && !is_infrastructure_topic(&topic.name)
        })
        .take(max_topics)
        .collect()
}

fn is_infrastructure_topic(topic: &str) -> bool {
    matches!(
        topic,
        "/parameter_events" | "/rosout" | "/tf" | "/tf_static" | "/clock"
    ) || topic.starts_with("/diagnostics")
}

fn observe_topic(topic: String, sample_window: Duration) -> RosTopicMetric {
    let hz_topic = topic.clone();
    let bw_topic = topic.clone();

    let hz_handle = thread::spawn(move || {
        run_sampling_command(&["topic", "hz", hz_topic.as_str()], sample_window)
    });
    let bw_handle = thread::spawn(move || {
        run_sampling_command(&["topic", "bw", bw_topic.as_str()], sample_window)
    });

    let hz_output = hz_handle
        .join()
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
    let bw_output = bw_handle
        .join()
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();

    let receive_frequency_hz = parse_hz(&hz_output);
    let (receive_bandwidth_bytes_per_sec, mean_message_bytes, sample_count) =
        parse_bandwidth(&bw_output);

    let populated = [
        receive_frequency_hz.is_some(),
        mean_message_bytes.is_some(),
        receive_bandwidth_bytes_per_sec.is_some(),
    ]
    .into_iter()
    .filter(|value| *value)
    .count();

    let confidence = match populated {
        3 => MetricConfidence::Observed,
        1 | 2 => MetricConfidence::Partial,
        _ => MetricConfidence::Unknown,
    };

    RosTopicMetric {
        topic,
        receive_frequency_hz,
        mean_message_bytes,
        receive_bandwidth_bytes_per_sec,
        sample_count,
        observation_window_ms: sample_window.as_millis(),
        source: "ros2_cli_hz+bw".to_owned(),
        confidence,
    }
}

fn run_sampling_command(args: &[&str], sample_window: Duration) -> io::Result<String> {
    let started = Instant::now();
    let mut child = Command::new("ros2")
        .args(args)
        .env("PYTHONUNBUFFERED", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    loop {
        match child.try_wait()? {
            Some(_) => {
                let output = child.wait_with_output()?;
                return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
            }
            None if started.elapsed() >= sample_window => {
                let _ = child.kill();
                let output = child.wait_with_output()?;
                return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    }
}

fn parse_hz(output: &str) -> Option<f64> {
    output.lines().rev().find_map(|line| {
        line.trim()
            .strip_prefix("average rate:")
            .and_then(|value| value.trim().parse::<f64>().ok())
    })
}

fn parse_bandwidth(output: &str) -> (Option<u64>, Option<u64>, Option<u64>) {
    let mut bandwidth = None;
    let mut mean_message_bytes = None;
    let mut sample_count = None;

    for line in output.lines() {
        let line = line.trim();

        if let Some((rate, rest)) = line.split_once("/s from ") {
            bandwidth = parse_bytes(rate.trim());
            sample_count = rest
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<u64>().ok());
        }

        if let Some(rest) = line.strip_prefix("Message size mean:") {
            let mut parts = rest.split_whitespace();
            if let (Some(value), Some(unit)) = (parts.next(), parts.next()) {
                mean_message_bytes = parse_bytes(&format!("{value} {unit}"));
            }
        }
    }

    (bandwidth, mean_message_bytes, sample_count)
}

fn parse_bytes(value: &str) -> Option<u64> {
    let mut parts = value.split_whitespace();
    let number = parts.next()?.parse::<f64>().ok()?;
    let unit = parts.next().unwrap_or("B").trim_end_matches("/s");
    let multiplier = match unit {
        "B" => 1.0,
        "KB" => 1_000.0,
        "MB" => 1_000_000.0,
        "GB" => 1_000_000_000.0,
        _ => return None,
    };
    Some((number * multiplier).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::{
        CliRosTopicMetricsCollector, MetricConfidence, parse_bandwidth, parse_hz,
        select_metric_topics,
    };
    use crate::ros2_probe::RosTopicInfo;
    use std::time::Duration;

    fn topic(name: &str, publishers: &[&str], subscribers: &[&str]) -> RosTopicInfo {
        RosTopicInfo {
            name: name.to_owned(),
            message_types: vec!["test_msgs/msg/Test".to_owned()],
            publishers: publishers.iter().map(|value| (*value).to_owned()).collect(),
            subscribers: subscribers.iter().map(|value| (*value).to_owned()).collect(),
        }
    }

    #[test]
    fn parses_ros2_topic_hz_output() {
        let output = "average rate: 9.995\n\tmin: 0.099s max: 0.101s std dev: 0.00050s window: 10\n";
        assert_eq!(parse_hz(output), Some(9.995));
    }

    #[test]
    fn parses_ros2_topic_bw_output_using_decimal_units() {
        let output = "5.20 MB/s from 100 messages\n\tMessage size mean: 0.08 MB min: 0.08 MB max: 0.08 MB\n";
        let (bandwidth, mean, samples) = parse_bandwidth(output);
        assert_eq!(bandwidth, Some(5_200_000));
        assert_eq!(mean, Some(80_000));
        assert_eq!(samples, Some(100));
    }

    #[test]
    fn selects_real_data_edges_and_ignores_infrastructure_topics() {
        let topics = vec![
            topic("/fastlio2/body_cloud", &["/fastlio2/lio_node"], &["/pgo/pgo_node"]),
            topic("/fastlio2/lio_odom", &["/fastlio2/lio_node"], &["/pgo/pgo_node"]),
            topic("/parameter_events", &["/a"], &["/b"]),
            topic("/rosout", &["/a"], &["/b"]),
            topic("/output_only", &["/a"], &[]),
        ];
        let selected = select_metric_topics(&topics, 2);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].name, "/fastlio2/body_cloud");
        assert_eq!(selected[1].name, "/fastlio2/lio_odom");
    }

    #[test]
    fn collector_builder_preserves_bounded_defaults() {
        let collector = CliRosTopicMetricsCollector::default()
            .with_sample_window(Duration::from_millis(500))
            .with_max_topics(1);
        let (metrics, _) = collector.collect(&[]);
        assert!(metrics.is_empty());
        let _ = MetricConfidence::Unknown;
    }
}
