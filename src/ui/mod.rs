mod dashboard;
mod detail;
pub mod model;

use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};

use crate::app::{Inspector, SortMode, UiState, ViewMode};
use crate::classifier::ProcessType;
use model::PresentationModel;

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const EVENT_POLL: Duration = Duration::from_millis(100);

pub fn run(initial_type_filter: Option<ProcessType>) -> io::Result<()> {
    let mut inspector = Inspector::default();
    let _ = inspector.refresh()?;
    std::thread::sleep(Duration::from_millis(250));
    let snapshot = inspector.refresh()?;
    let mut state = UiState {
        process_type_filter: initial_type_filter,
        ..UiState::default()
    };
    let mut model = PresentationModel::new(snapshot, &mut state);
    let mut last_refresh = Instant::now();

    ratatui::run(|terminal| {
        let mut dirty = true;
        loop {
            if dirty {
                terminal.draw(|frame| dashboard::render(frame, &model, &state))?;
                dirty = false;
            }

            let timeout = next_poll_timeout(last_refresh.elapsed());
            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if handle_key(&mut state, &mut model, key) {
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
                    model.integrate_snapshot(next, &mut state, true);
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

fn handle_key(state: &mut UiState, model: &mut PresentationModel, key: KeyEvent) -> bool {
    if state.search_active {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => state.search_active = false,
            KeyCode::Backspace => {
                state.query.pop();
                model.reorder(state);
            }
            KeyCode::Char(character) => {
                state.query.push(character);
                model.reorder(state);
            }
            _ => {}
        }
        return false;
    }

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
                model.reorder(state);
            }
            false
        }
        KeyCode::Down | KeyCode::Char('j') => {
            model.move_selection(state, 1);
            false
        }
        KeyCode::Up | KeyCode::Char('k') => {
            model.move_selection(state, -1);
            false
        }
        KeyCode::Enter => {
            state.open_detail();
            false
        }
        KeyCode::Char('/') if state.view == ViewMode::List => {
            state.search_active = true;
            false
        }
        KeyCode::Char('t') if state.view == ViewMode::List => {
            state.tree_mode = !state.tree_mode;
            model.reorder(state);
            false
        }
        KeyCode::Char('c') if state.view == ViewMode::List => {
            state.sort = SortMode::Cpu;
            state.tree_mode = false;
            model.reorder(state);
            false
        }
        KeyCode::Char('m') if state.view == ViewMode::List => {
            state.sort = SortMode::Memory;
            state.tree_mode = false;
            model.reorder(state);
            false
        }
        KeyCode::Char('g') if state.view == ViewMode::List => {
            state.sort = SortMode::Gpu;
            state.tree_mode = false;
            model.reorder(state);
            false
        }
        KeyCode::Char('p') if state.view == ViewMode::List => {
            state.sort = SortMode::Pid;
            state.tree_mode = false;
            model.reorder(state);
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
