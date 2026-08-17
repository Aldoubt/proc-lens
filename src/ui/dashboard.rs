use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Row, Table, TableState};

use crate::app::{AppSnapshot, SortMode, UiState, ViewMode, format_bytes, project_label};

pub fn render(frame: &mut Frame, snapshot: &AppSnapshot, state: &UiState) {
    if state.view == ViewMode::Detail {
        super::detail::render(frame, snapshot, state);
        return;
    }

    let [title_area, metrics_area, table_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let mode = if state.tree_mode { "tree" } else { sort_label(state.sort) };
    let title = Paragraph::new(format!(
        " proc-lens {}  load {:.2} {:.2} {:.2}  processes {}  mode {}",
        env!("CARGO_PKG_VERSION"),
        snapshot.load_average[0],
        snapshot.load_average[1],
        snapshot.load_average[2],
        snapshot.processes.len(),
        mode,
    ));
    frame.render_widget(title, title_area);

    let gpu_device = snapshot.gpu.as_ref().and_then(|gpu| gpu.devices.first());
    let metric_areas = if gpu_device.is_some() {
        Layout::horizontal([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(metrics_area)
    } else {
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(metrics_area)
    };

    let cpu = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("CPU"))
        .ratio((snapshot.cpu_percent / 100.0).clamp(0.0, 1.0) as f64)
        .label(format!("{:.1}%", snapshot.cpu_percent));
    frame.render_widget(cpu, metric_areas[0]);

    let memory = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("RAM"))
        .ratio((snapshot.memory.used_percent() / 100.0).clamp(0.0, 1.0) as f64)
        .label(format!(
            "{} / {}",
            format_bytes(snapshot.memory.used_bytes()),
            format_bytes(snapshot.memory.total_bytes)
        ));
    frame.render_widget(memory, metric_areas[1]);

    if let Some(device) = gpu_device {
        let ratio = device
            .utilization_percent
            .map(|value| (value / 100.0).clamp(0.0, 1.0) as f64)
            .unwrap_or(0.0);
        let mut label = device
            .utilization_percent
            .map(|value| format!("{value:.1}%"))
            .unwrap_or_else(|| "util -".into());
        if let (Some(used), Some(total)) = (device.memory_used_bytes, device.memory_total_bytes) {
            label.push_str(&format!("  {} / {}", format_bytes(used), format_bytes(total)));
        }
        if let Some(temp) = device.temperature_c {
            label.push_str(&format!("  {temp}C"));
        }
        let gpu = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("GPU{}", device.index)),
            )
            .ratio(ratio)
            .label(label);
        frame.render_widget(gpu, metric_areas[2]);
    }

    let visible = state.visible_processes(snapshot);
    let rows = visible.iter().map(|process| {
        let command = if state.tree_mode {
            format!(
                "{}{}",
                "  ".repeat(process.tree_depth),
                process.snapshot.name
            )
        } else {
            process.snapshot.command_line()
        };
        Row::new(vec![
            process.snapshot.pid.to_string(),
            process.classification.process_type.to_string(),
            project_label(process),
            format!("{:.1}", process.snapshot.cpu_percent),
            format_bytes(process.snapshot.memory_bytes),
            process
                .snapshot
                .gpu
                .as_ref()
                .and_then(|gpu| gpu.utilization_percent)
                .map(|value| format!("{value:.0}"))
                .unwrap_or_else(|| "-".into()),
            process
                .snapshot
                .gpu
                .as_ref()
                .and_then(|gpu| gpu.vram_bytes)
                .map(format_bytes)
                .unwrap_or_else(|| "-".into()),
            command,
        ])
    });

    let header = Row::new(vec!["PID", "TYPE", "PROJECT", "CPU%", "RAM", "GPU%", "VRAM", "COMMAND"])
        .style(Style::default().add_modifier(Modifier::BOLD));
    let table = Table::new(
        rows,
        [
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Length(24),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .column_spacing(1)
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol("> ");

    let mut table_state = TableState::default().with_selected((!visible.is_empty()).then_some(state.selected));
    frame.render_stateful_widget(table, table_area, &mut table_state);

    let footer = if state.search_active {
        format!(" /{}", state.query)
    } else {
        format!(
            " j/k move | Enter detail | / search{} | t tree | c cpu | m mem | g gpu | q quit",
            if state.query.is_empty() {
                String::new()
            } else {
                format!(" [{}]", state.query)
            }
        )
    };
    frame.render_widget(Paragraph::new(footer), footer_area);
}

fn sort_label(sort: SortMode) -> &'static str {
    match sort {
        SortMode::Cpu => "cpu",
        SortMode::Memory => "memory",
        SortMode::Gpu => "gpu",
        SortMode::Pid => "pid",
    }
}
