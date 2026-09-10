//! ROS 2 topic throughput observation model.

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
