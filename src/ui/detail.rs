use ratatui::Frame;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::{AppSnapshot, UiState, format_inspect};

pub fn render(frame: &mut Frame, snapshot: &AppSnapshot, state: &UiState) {
    let visible = state.visible_processes(snapshot);
    let text = visible
        .get(state.selected)
        .and_then(|process| format_inspect(snapshot, process.snapshot.pid))
        .unwrap_or_else(|| "Process is no longer available".into());

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Process detail — q/Esc back"),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, frame.area());
}
