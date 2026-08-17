from pathlib import Path

path = Path("src/app.rs")
text = path.read_text()

start_marker = "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct UiState {"
end_marker = "pub struct Inspector {"
start = text.index(start_marker)
end = text.index(end_marker, start)

replacement = '''#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiState {
    pub selected_pid: Option<i32>,
    pub query: String,
    pub search_active: bool,
    pub process_type_filter: Option<ProcessType>,
    pub tree_mode: bool,
    pub sort: SortMode,
    pub view: ViewMode,
    pub paused: bool,
    pub help_open: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            selected_pid: None,
            query: String::new(),
            search_active: false,
            process_type_filter: None,
            tree_mode: false,
            sort: SortMode::Cpu,
            view: ViewMode::List,
            paused: false,
            help_open: false,
        }
    }
}

impl UiState {
    pub fn open_detail(&mut self) {
        if self.selected_pid.is_some() {
            self.view = ViewMode::Detail;
        }
    }

    pub fn back(&mut self) {
        self.view = ViewMode::List;
    }
}

'''

path.write_text(text[:start] + replacement + text[end:])
