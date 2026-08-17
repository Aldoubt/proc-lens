use proc_lens::app::{UiState, ViewMode};

#[test]
fn ui_state_defaults_to_no_selected_pid() {
    let state = UiState::default();

    assert_eq!(state.selected_pid, None);
    assert_eq!(state.view, ViewMode::List);
}

#[test]
fn detail_requires_a_selected_pid() {
    let mut state = UiState::default();

    state.open_detail();

    assert_eq!(state.view, ViewMode::List);
}

#[test]
fn detail_opens_for_selected_pid_and_back_returns_to_list() {
    let mut state = UiState {
        selected_pid: Some(42),
        ..UiState::default()
    };

    state.open_detail();
    assert_eq!(state.view, ViewMode::Detail);

    state.back();
    assert_eq!(state.view, ViewMode::List);
    assert_eq!(state.selected_pid, Some(42));
}
