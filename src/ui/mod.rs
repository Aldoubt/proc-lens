mod dashboard;
mod detail;
pub mod model;

use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};

use crate::app::{EntityMode, Inspector, SortMode, UiState, ViewMode};
use crate::classifier::ProcessType;
use model::PresentationModel;

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const REORDER_INTERVAL: Duration = Duration::from_secs(2);
const EVENT_POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TickAction {
    None,
    Sample,
    Reorder,
    SampleAndReorder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputAction {
    None,
    Quit,
    RefreshNow,
    ResumeRefresh,
}

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
    let mut last_sample = Instant::now();
    let mut last_reorder = Instant::now();

    ratatui::run(|terminal| {
        let mut dirty = true;
        loop {
            if dirty {
                terminal.draw(|frame| dashboard::render(frame, &model, &state))?;
                dirty = false;
            }

            let timeout = if state.paused {
                EVENT_POLL
            } else {
                next_poll_timeout(last_sample.elapsed())
            };
            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        let action = handle_key(&mut state, &mut model, key, terminal_page_size());
                        match action {
                            InputAction::Quit => break Ok(()),
                            InputAction::RefreshNow | InputAction::ResumeRefresh => {
                                if let Ok(next) = inspector.refresh() {
                                    model.integrate_snapshot(next, &mut state, true);
                                }
                                last_sample = Instant::now();
                                last_reorder = last_sample;
                            }
                            InputAction::None => {}
                        }
                        dirty = true;
                    }
                    Event::Resize(_, _) => dirty = true,
                    _ => {}
                }
            }

            match automatic_action(state.paused, last_sample.elapsed(), last_reorder.elapsed()) {
                TickAction::None => {}
                TickAction::Sample => {
                    if let Ok(next) = inspector.refresh() {
                        model.integrate_snapshot(next, &mut state, false);
                        dirty = true;
                    }
                    last_sample = Instant::now();
                }
                TickAction::Reorder => {
                    model.reorder(&mut state);
                    last_reorder = Instant::now();
                    dirty = true;
                }
                TickAction::SampleAndReorder => {
                    if let Ok(next) = inspector.refresh() {
                        model.integrate_snapshot(next, &mut state, true);
                    } else {
                        model.reorder(&mut state);
                    }
                    let now = Instant::now();
                    last_sample = now;
                    last_reorder = now;
                    dirty = true;
                }
            }
        }
    })
}

fn next_poll_timeout(elapsed: Duration) -> Duration {
    SAMPLE_INTERVAL.saturating_sub(elapsed).min(EVENT_POLL)
}

fn automatic_action(
    paused: bool,
    sample_elapsed: Duration,
    reorder_elapsed: Duration,
) -> TickAction {
    if paused {
        return TickAction::None;
    }

    match (
        sample_elapsed >= SAMPLE_INTERVAL,
        reorder_elapsed >= REORDER_INTERVAL,
    ) {
        (true, true) => TickAction::SampleAndReorder,
        (true, false) => TickAction::Sample,
        (false, true) => TickAction::Reorder,
        (false, false) => TickAction::None,
    }
}

fn toggle_pause(state: &mut UiState) -> InputAction {
    state.paused = !state.paused;
    if state.paused {
        InputAction::None
    } else {
        InputAction::ResumeRefresh
    }
}

fn manual_refresh_action(state: &UiState) -> InputAction {
    if state.paused {
        InputAction::RefreshNow
    } else {
        InputAction::None
    }
}

fn terminal_page_size() -> usize {
    crossterm::terminal::size()
        .map(|(_, rows)| usize::from(rows.saturating_sub(6)).max(1))
        .unwrap_or(10)
}

fn current_selected_index(model: &PresentationModel, state: &UiState) -> Option<usize> {
    match state.entity_mode {
        EntityMode::Process => model.selected_index(state),
        EntityMode::Task => model.selected_task_index(state),
    }
}

fn reorder_current_preserving_index(
    model: &mut PresentationModel,
    state: &mut UiState,
    previous_index: Option<usize>,
) {
    match state.entity_mode {
        EntityMode::Process => model.reorder_preserving_index(state, previous_index),
        EntityMode::Task => model.reorder_tasks_preserving_index(state, previous_index),
    }
}

fn move_selection(model: &PresentationModel, state: &mut UiState, delta: isize) {
    match state.entity_mode {
        EntityMode::Process => model.move_selection(state, delta),
        EntityMode::Task => model.move_task_selection(state, delta),
    }
}

fn move_page(model: &PresentationModel, state: &mut UiState, page_size: usize, direction: isize) {
    match state.entity_mode {
        EntityMode::Process => model.move_page(state, page_size, direction),
        EntityMode::Task => model.move_task_page(state, page_size, direction),
    }
}

fn select_first(model: &PresentationModel, state: &mut UiState) {
    match state.entity_mode {
        EntityMode::Process => model.select_first(state),
        EntityMode::Task => model.select_first_task(state),
    }
}

fn select_last(model: &PresentationModel, state: &mut UiState) {
    match state.entity_mode {
        EntityMode::Process => model.select_last(state),
        EntityMode::Task => model.select_last_task(state),
    }
}

fn handle_key(
    state: &mut UiState,
    model: &mut PresentationModel,
    key: KeyEvent,
    page_size: usize,
) -> InputAction {
    if state.search_active {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => state.search_active = false,
            KeyCode::Backspace => {
                let previous_index = current_selected_index(model, state);
                state.query.pop();
                reorder_current_preserving_index(model, state, previous_index);
            }
            KeyCode::Char(character) => {
                let previous_index = current_selected_index(model, state);
                state.query.push(character);
                reorder_current_preserving_index(model, state, previous_index);
            }
            _ => {}
        }
        return InputAction::None;
    }

    if state.help_open {
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?')
        ) {
            state.help_open = false;
        }
        return InputAction::None;
    }

    match key.code {
        KeyCode::Char('q') => {
            if state.view == ViewMode::Detail {
                state.back();
                InputAction::None
            } else {
                InputAction::Quit
            }
        }
        KeyCode::Esc => {
            if state.view == ViewMode::Detail {
                state.back();
            } else if !state.query.is_empty() {
                let previous_index = current_selected_index(model, state);
                state.query.clear();
                reorder_current_preserving_index(model, state, previous_index);
            }
            InputAction::None
        }
        KeyCode::Char(' ') => toggle_pause(state),
        KeyCode::Char('r') => manual_refresh_action(state),
        KeyCode::Char('?') => {
            state.help_open = true;
            InputAction::None
        }
        KeyCode::Char('a') if state.view == ViewMode::List => {
            state.toggle_entity_mode();
            match state.entity_mode {
                EntityMode::Process if state.selected_pid.is_none() => model.select_first(state),
                EntityMode::Task if state.selected_task_id.is_none() => model.select_first_task(state),
                _ => {}
            }
            InputAction::None
        }
        KeyCode::Down | KeyCode::Char('j') if state.view == ViewMode::List => {
            move_selection(model, state, 1);
            InputAction::None
        }
        KeyCode::Up | KeyCode::Char('k') if state.view == ViewMode::List => {
            move_selection(model, state, -1);
            InputAction::None
        }
        KeyCode::PageDown if state.view == ViewMode::List => {
            move_page(model, state, page_size, 1);
            InputAction::None
        }
        KeyCode::PageUp if state.view == ViewMode::List => {
            move_page(model, state, page_size, -1);
            InputAction::None
        }
        KeyCode::Home if state.view == ViewMode::List => {
            select_first(model, state);
            InputAction::None
        }
        KeyCode::End if state.view == ViewMode::List => {
            select_last(model, state);
            InputAction::None
        }
        KeyCode::Enter => {
            state.open_detail();
            InputAction::None
        }
        KeyCode::Char('/') if state.view == ViewMode::List => {
            state.search_active = true;
            InputAction::None
        }
        KeyCode::Char('t')
            if state.view == ViewMode::List && state.entity_mode == EntityMode::Process =>
        {
            state.tree_mode = !state.tree_mode;
            model.reorder(state);
            InputAction::None
        }
        KeyCode::Char('c') if state.view == ViewMode::List => {
            state.sort = SortMode::Cpu;
            state.tree_mode = false;
            model.reorder(state);
            InputAction::None
        }
        KeyCode::Char('m') if state.view == ViewMode::List => {
            state.sort = SortMode::Memory;
            state.tree_mode = false;
            model.reorder(state);
            InputAction::None
        }
        KeyCode::Char('g') if state.view == ViewMode::List => {
            state.sort = SortMode::Gpu;
            state.tree_mode = false;
            model.reorder(state);
            InputAction::None
        }
        KeyCode::Char('p') if state.view == ViewMode::List => {
            state.sort = SortMode::Pid;
            state.tree_mode = false;
            model.reorder(state);
            InputAction::None
        }
        _ => InputAction::None,
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

    #[test]
    fn paused_automatic_tick_does_nothing() {
        assert_eq!(
            automatic_action(true, Duration::from_secs(10), Duration::from_secs(10)),
            TickAction::None
        );
    }

    #[test]
    fn automatic_tick_separates_sampling_and_reordering() {
        assert_eq!(
            automatic_action(false, Duration::from_secs(1), Duration::from_secs(1)),
            TickAction::Sample
        );
        assert_eq!(
            automatic_action(false, Duration::from_millis(500), Duration::from_secs(2)),
            TickAction::Reorder
        );
        assert_eq!(
            automatic_action(false, Duration::from_secs(1), Duration::from_secs(2)),
            TickAction::SampleAndReorder
        );
    }

    #[test]
    fn pause_transition_requests_refresh_only_on_resume() {
        let mut state = UiState::default();

        assert_eq!(toggle_pause(&mut state), InputAction::None);
        assert!(state.paused);

        assert_eq!(toggle_pause(&mut state), InputAction::ResumeRefresh);
        assert!(!state.paused);
    }

    #[test]
    fn manual_refresh_is_only_requested_while_paused() {
        let mut state = UiState::default();
        assert_eq!(manual_refresh_action(&state), InputAction::None);

        state.paused = true;
        assert_eq!(manual_refresh_action(&state), InputAction::RefreshNow);
        assert!(state.paused);
    }
}
