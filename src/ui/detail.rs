use ratatui::Frame;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::{AppSnapshot, EntityMode, UiState, format_inspect};
use crate::task::format_task;

pub fn render(frame: &mut Frame, snapshot: &AppSnapshot, state: &UiState) {
    let (title, text) = match state.entity_mode {
        EntityMode::Process => (
            "Process detail — q/Esc back",
            state
                .selected_pid
                .and_then(|pid| format_inspect(snapshot, pid))
                .unwrap_or_else(|| "Process is no longer available".into()),
        ),
        EntityMode::Task => (
            "Task detail — q/Esc back",
            state
                .selected_task_id
                .as_ref()
                .and_then(|task_id| format_task(snapshot, task_id))
                .unwrap_or_else(|| "Task is no longer available".into()),
        ),
    };

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, frame.area());
}
