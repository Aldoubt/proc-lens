use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Gauge, Paragraph, Row, Table, TableState, Wrap};

use crate::app::{SortMode, UiState, ViewMode, format_bytes, project_label};

use super::model::{PresentationModel, compact_command_label};

const HELP_TEXT: &str = "↑ / k        previous process\n\
↓ / j        next process\n\
PgUp/PgDn   move one page\n\
Home/End    first / last process\n\
Enter       inspect selected PID\n\
/           search\n\
Space       pause / resume\n\
r           refresh once while paused\n\
t           tree mode\n\
c / m       sort CPU / memory\n\
g / p       sort GPU / PID\n\
?           toggle this help\n\
q / Esc     back / close";

pub fn render(frame: &mut Frame, model: &PresentationModel, state: &UiState) {
    let snapshot = model.snapshot();
    if state.view == ViewMode::Detail {
        super::detail::render(frame, snapshot, state);
        if state.help_open {
            render_help(frame);
        }
        return;
    }

    let [title_area, metrics_area, table_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let mode = if state.tree_mode {
        "tree"
    } else {
        sort_label(state.sort)
    };
    let status = if state.paused { "  PAUSED" } else { "" };
    let title = Paragraph::new(format!(
        " proc-lens {}  load {:.2} {:.2} {:.2}  processes {}  mode {}{}",
        env!("CARGO_PKG_VERSION"),
        snapshot.load_average[0],
        snapshot.load_average[1],
        snapshot.load_average[2],
        snapshot.processes.len(),
        mode,
        status,
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
            label.push_str(&format!(
                "  {} / {}",
                format_bytes(used),
                format_bytes(total)
            ));
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

    let command_budget = usize::from(frame.area().width.saturating_sub(88)).max(20);
    let visible = model.visible_rows(state);
    let rows = visible.iter().map(|row| {
        let process = row.process;
        let command = if state.tree_mode {
            format!(
                "{}{}",
                "  ".repeat(process.tree_depth),
                process.snapshot.name
            )
        } else {
            compact_command_label(process, command_budget)
        };
        Row::new(vec![
            process.snapshot.pid.to_string(),
            process.classification.process_type.to_string(),
            project_label(process),
            format!("{:.1}", row.cpu_percent),
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

    let header = Row::new(vec![
        "PID", "TYPE", "PROJECT", "CPU%", "RAM", "GPU%", "VRAM", "COMMAND",
    ])
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

    let mut table_state = TableState::default().with_selected(model.selected_index(state));
    frame.render_stateful_widget(table, table_area, &mut table_state);

    let footer = if state.search_active {
        format!(" /{}  Enter/Esc done", state.query)
    } else {
        " ↑↓ move  Enter inspect  / search  Space pause  ? help".to_owned()
    };
    frame.render_widget(Paragraph::new(footer), footer_area);

    if state.help_open {
        render_help(frame);
    }
}

fn render_help(frame: &mut Frame) {
    let [_, middle, _] = Layout::vertical([
        Constraint::Percentage(15),
        Constraint::Percentage(70),
        Constraint::Percentage(15),
    ])
    .areas(frame.area());
    let [_, area, _] = Layout::horizontal([
        Constraint::Percentage(20),
        Constraint::Percentage(60),
        Constraint::Percentage(20),
    ])
    .areas(middle);

    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(HELP_TEXT)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("proc-lens help — ?/Esc/q close"),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn sort_label(sort: SortMode) -> &'static str {
    match sort {
        SortMode::Cpu => "cpu",
        SortMode::Memory => "memory",
        SortMode::Gpu => "gpu",
        SortMode::Pid => "pid",
    }
}
