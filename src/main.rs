use std::error::Error;
use std::io;
use std::thread;
use std::time::Duration;

use clap::{Parser, Subcommand};
use proc_lens::app::{Inspector, format_inspect, format_snapshot};
use proc_lens::classifier::ProcessType;
use proc_lens::task::{TaskId, format_task, format_tasks};

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
    Snapshot,
    /// Explain one process and its provenance.
    Inspect { pid: i32 },
    /// Print a non-interactive task-level resource snapshot.
    Tasks,
    /// Explain one current task and list its member processes.
    Task { task_id: String },
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
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
        Some(Command::Snapshot) => {
            let snapshot = sampled_snapshot()?;
            let output = format_snapshot(&snapshot, cli.process_type);
            print!("{output}");
        }
        Some(Command::Tasks) => {
            let snapshot = sampled_snapshot()?;
            print!("{}", format_tasks(&snapshot));
        }
        Some(Command::Task { task_id }) => {
            let snapshot = sampled_snapshot()?;
            let task_id = TaskId::new(task_id);
            let output = format_task(&snapshot, &task_id).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("task {task_id} is not available"),
                )
            })?;
            print!("{output}");
        }
        None => proc_lens::ui::run(cli.process_type)?,
    }

    Ok(())
}

fn sampled_snapshot() -> io::Result<proc_lens::app::AppSnapshot> {
    let mut inspector = Inspector::default();
    let _ = inspector.refresh()?;
    thread::sleep(Duration::from_millis(250));
    inspector.refresh()
}
