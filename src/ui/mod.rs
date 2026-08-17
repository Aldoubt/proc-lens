mod dashboard;
mod detail;

use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};

use crate::app::{Inspector, SortMode, UiState, ViewMode};
use crate::classifier::ProcessType;

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const EVENT_POLL: Duration = Duration::from_millis(100);

pub fn run(initial_type_filter: Option<ProcessType>) -> io::Result<()> {
    let mut inspector = Inspector::default();
    let _ = inspector.refresh()?;
    std::thread::sleep(Duration::from_millis(250));
    let mut snapshot = inspector.refresh()?;
    let mut state = UiState {
        process_type_filter: initial_type_filter,
        ..UiState::default()
    };
    let mut last_refresh = Instant::now();

    ratatui::run(|terminal| {
        let mut dirty = true;
        loop {
            if dirty {
                let visible_len = state.visible_processes(&snapshot).len();
                state.clamp_selection(visible_len);
                terminal.draw(|frame| dashboard::render(frame, &snapshot, &state))?;
                dirty = false;
            }

            let timeout = next_poll_timeout(last_refresh.elapsed());
            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if handle_key(&mut state, &snapshot, key) {
                            break Ok(());
                        }
                        dirty = true;
                    }
                    Event::Resize(_, _) => dirty = true,
                    _ => {}
                }
            }

            if last_refresh.elapsed() >= REFRESH_INTERVAL {
                if let Ok(next) = inspector.refresh() {
                    snapshot = next;
                }
                last_refresh = Instant::now();
                dirty = true;
            }
        }
    })
}

fn next_poll_timeout(elapsed: Duration) -> Duration {
    REFRESH_INTERVAL.saturating_sub(elapsed).min(EVENT_POLL)
}

fn handle_key(state: &mut UiState, snapshot: &crate::app::AppSnapshot, key: KeyEvent) -> bool {
    if state.search_active {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => state.search_active = false,
            KeyCode::Backspace => {
                state.query.pop();
                state.selected = 0;
            }
            KeyCode::Char(character) => {
                state.query.push(character);
                state.selected = 0;
            }
            _ => {}
        }
        return false;
    }

    let visible_len = state.visible_processes(snapshot).len();
    match key.code {
        KeyCode::Char('q') => {
            if state.view == ViewMode::Detail {
                state.back();
                false
            } else {
                true
            }
        }
        KeyCode::Esc => {
            if state.view == ViewMode::Detail {
                state.back();
            } else if !state.query.is_empty() {
                state.query.clear();
                state.selected = 0;
            }
            false
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.move_down(visible_len);
            false
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.move_up();
            false
        }
        KeyCode::Enter => {
            state.open_detail(visible_len);
            false
        }
        KeyCode::Char('/') if state.view == ViewMode::List => {
            state.search_active = true;
            false
        }
        KeyCode::Char('t') if state.view == ViewMode::List => {
            state.tree_mode = !state.tree_mode;
            state.selected = 0;
            false
        }
        KeyCode::Char('c') if state.view == ViewMode::List => {
            state.sort = SortMode::Cpu;
            state.tree_mode = false;
            state.selected = 0;
            false
        }
        KeyCode::Char('m') if state.view == ViewMode::List => {
            state.sort = SortMode::Memory;
            state.tree_mode = false;
            state.selected = 0;
            false
        }
        KeyCode::Char('g') if state.view == ViewMode::List => {
            state.sort = SortMode::Gpu;
            state.tree_mode = false;
            state.selected = 0;
            false
        }
        KeyCode::Char('p') if state.view == ViewMode::List => {
            state.sort = SortMode::Pid;
            state.tree_mode = false;
            state.selected = 0;
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_poll_timeout_never_waits_past_refresh_deadline() {
        assert_eq!(
            next_poll_timeout(Duration::from_millis(900)),
            Duration::from_millis(100)
        );
        assert_eq!(
            next_poll_timeout(Duration::from_millis(950)),
            Duration::from_millis(50)
        );
        assert_eq!(next_poll_timeout(Duration::from_secs(1)), Duration::ZERO);
    }
}
