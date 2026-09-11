use std::error::Error;
use std::io;
use std::thread;
use std::time::Duration;

use clap::{Parser, Subcommand};
use proc_lens::app::{Inspector, format_inspect, format_snapshot};
use proc_lens::classifier::ProcessType;
use proc_lens::collector::thread::ThreadCollector;
use proc_lens::ros2_probe::{
    CliRosGraphProvider, CliRosTopicMetricsCollector, CliRosTopicTopologyProvider, RosGraphProvider,
    RosTopicTopologyProvider,
};
use proc_lens::runtime::RuntimeSnapshot;

#[derive(Debug, Parser)]
#[command(name = "proc-lens", version, about)]
struct Cli {
    /// Show only one process category (ros2, docker, systemd, dev, browser, process).
    #[arg(long = "type")]
    process_type: Option<ProcessType>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print a non-interactive process snapshot.
    Snapshot {
        /// Emit the normalized runtime snapshot as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Explain one process and its provenance.
    Inspect { pid: i32 },
}

fn main() -> Result<(), Box<dyn Error>> {
    let Cli {
        process_type,
        command,
    } = Cli::parse();

    match command {
        Some(Command::Inspect { pid }) => {
            let snapshot = sampled_snapshot()?;
            let output = format_inspect(&snapshot, pid).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("process {pid} is not available"),
                )
            })?;
            println!("{output}");
        }
        Some(Command::Snapshot { json: true }) => {
            let snapshot = sampled_runtime_snapshot(process_type)?;
            println!("{}", snapshot.to_json_pretty()?);
        }
        Some(Command::Snapshot { json: false }) => {
            let snapshot = sampled_snapshot()?;
            let output = format_snapshot(&snapshot, process_type);
            print!("{output}");
        }
        None => proc_lens::ui::run(process_type)?,
    }

    Ok(())
}

fn sampled_snapshot() -> io::Result<proc_lens::app::AppSnapshot> {
    let mut inspector = Inspector::default();
    let _ = inspector.refresh()?;
    thread::sleep(Duration::from_millis(250));
    inspector.refresh()
}

fn sampled_runtime_snapshot(filter: Option<ProcessType>) -> io::Result<RuntimeSnapshot> {
    let mut inspector = Inspector::default();
    let mut thread_collector = ThreadCollector::default();

    let first = inspector.refresh()?;
    let first_processes = first
        .processes
        .iter()
        .map(|process| process.snapshot.clone())
        .collect::<Vec<_>>();
    let _ = thread_collector.sample(&first_processes)?;

    thread::sleep(Duration::from_millis(250));

    let snapshot = inspector.refresh()?;
    let processes = snapshot
        .processes
        .iter()
        .map(|process| process.snapshot.clone())
        .collect::<Vec<_>>();
    let threads = thread_collector.sample(&processes)?;

    let (ros_nodes, graph_elapsed) = CliRosGraphProvider::default()
        .discover()
        .map(|(nodes, elapsed)| (nodes, Some(elapsed.as_millis())))
        .unwrap_or_default();

    let (ros_topics, ros_edges, topology_elapsed) = CliRosTopicTopologyProvider::default()
        .discover(&ros_nodes)
        .map(|(topics, edges, elapsed)| (topics, edges, Some(elapsed.as_millis())))
        .unwrap_or_default();

    let (ros_topic_metrics, metrics_elapsed) = if ros_topics.is_empty() {
        (Vec::new(), None)
    } else {
        let (metrics, elapsed) = CliRosTopicMetricsCollector::default().collect(&ros_topics);
        (metrics, Some(elapsed.as_millis()))
    };

    Ok(RuntimeSnapshot::from_app(
        &snapshot,
        &threads,
        filter,
        &ros_nodes,
        &ros_topics,
        &ros_edges,
        &ros_topic_metrics,
        graph_elapsed,
        topology_elapsed,
        metrics_elapsed,
    ))
}
