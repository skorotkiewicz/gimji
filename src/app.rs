use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::Local;
use directories::ProjectDirs;
use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::models::{
    CalendarData, CalendarEvent, KanbanBoard, KanbanCard, TabContent, TabType, TodoItem, TodoList,
};
use crate::storage::Workspace;

const AUTOSAVE_AFTER: Duration = Duration::from_millis(700);

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Gimji",
        options,
        Box::new(|creation_context| Ok(Box::new(GimjiApp::new(creation_context)))),
    )
}

#[derive(Debug)]
struct GimjiApp {
    workspace: Option<Workspace>,
    loaded: Option<LoadedTab>,
    recent: RecentWorkspaces,
    note_filter: String,
    new_note_title: String,
    new_tab_title: String,
    new_tab_type: TabType,
    rename_note_title: String,
    rename_tab_title: String,
    save_status: SaveStatus,
    pending_confirm: Option<ConfirmAction>,
    message: Option<String>,
}

impl GimjiApp {
    fn new(_creation_context: &eframe::CreationContext<'_>) -> Self {
        Self {
            workspace: None,
            loaded: None,
            recent: RecentWorkspaces::load(),
            note_filter: String::new(),
            new_note_title: "New Note".to_owned(),
            new_tab_title: "Markdown".to_owned(),
            new_tab_type: TabType::Markdown,
            rename_note_title: String::new(),
            rename_tab_title: String::new(),
            save_status: SaveStatus::Idle,
            pending_confirm: None,
            message: None,
        }
    }

    fn open_workspace_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.open_workspace(path);
        }
    }

    fn new_workspace_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.create_workspace(path);
        }
    }

    fn open_workspace(&mut self, path: PathBuf) {
        self.flush_current();
        match Workspace::open(&path) {
            Ok(workspace) => {
                self.workspace = Some(workspace);
                self.loaded = None;
                self.recent.add(path);
                self.load_selected_content();
            }
            Err(error) => self.set_error(error.to_string()),
        }
    }

    fn create_workspace(&mut self, path: PathBuf) {
        self.flush_current();
        match Workspace::create(&path) {
            Ok(workspace) => {
                self.workspace = Some(workspace);
                self.loaded = None;
                self.recent.add(path);
                self.load_selected_content();
            }
            Err(error) => self.set_error(error.to_string()),
        }
    }

    fn add_note(&mut self) {
        let title = self.new_note_title.trim().to_owned();
        if title.is_empty() {
            return;
        }

        self.flush_current();
        if let Some(workspace) = &mut self.workspace {
            match workspace.add_note(&title) {
                Ok(_) => {
                    self.new_note_title = "New Note".to_owned();
                    self.loaded = None;
                    self.load_selected_content();
                }
                Err(error) => self.set_error(error.to_string()),
            }
        }
    }

    fn add_tab(&mut self) {
        let title = self.new_tab_title.trim().to_owned();
        if title.is_empty() {
            return;
        }

        let Some(note_id) = self
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.selected_note_id())
            .map(str::to_owned)
        else {
            return;
        };

        self.flush_current();
        if let Some(workspace) = &mut self.workspace {
            match workspace.add_tab(&note_id, &title, self.new_tab_type) {
                Ok(_) => {
                    self.new_tab_title = self.new_tab_type.label().to_owned();
                    self.loaded = None;
                    self.load_selected_content();
                }
                Err(error) => self.set_error(error.to_string()),
            }
        }
    }

    fn rename_current_note(&mut self) {
        let title = self.rename_note_title.trim().to_owned();
        if title.is_empty() {
            return;
        }

        let Some(note_id) = self
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.selected_note_id())
            .map(str::to_owned)
        else {
            return;
        };

        if let Some(workspace) = &mut self.workspace
            && let Err(error) = workspace.rename_note(&note_id, &title)
        {
            self.set_error(error.to_string());
        }
    }

    fn rename_current_tab(&mut self) {
        let title = self.rename_tab_title.trim().to_owned();
        if title.is_empty() {
            return;
        }

        let Some(tab_id) = self
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.selected_tab_id())
            .map(str::to_owned)
        else {
            return;
        };

        if let Some(workspace) = &mut self.workspace
            && let Err(error) = workspace.rename_tab(&tab_id, &title)
        {
            self.set_error(error.to_string());
        }
    }

    fn select_note(&mut self, note_id: String) {
        self.flush_current();
        if let Some(workspace) = &mut self.workspace {
            match workspace.select_note(&note_id) {
                Ok(()) => {
                    self.loaded = None;
                    self.load_selected_content();
                }
                Err(error) => self.set_error(error.to_string()),
            }
        }
    }

    fn select_tab(&mut self, tab_id: String) {
        self.flush_current();
        if let Some(workspace) = &mut self.workspace {
            match workspace.select_tab(&tab_id) {
                Ok(()) => {
                    self.loaded = None;
                    self.load_selected_content();
                }
                Err(error) => self.set_error(error.to_string()),
            }
        }
    }

    fn load_selected_content(&mut self) {
        let Some(workspace) = &self.workspace else {
            self.save_status = SaveStatus::Idle;
            return;
        };

        let Some(tab_id) = workspace.selected_tab_id().map(str::to_owned) else {
            self.loaded = None;
            self.save_status = SaveStatus::Idle;
            return;
        };

        if self
            .loaded
            .as_ref()
            .is_some_and(|loaded| loaded.tab_id == tab_id)
        {
            return;
        }

        match workspace.load_tab_content(&tab_id) {
            Ok(content) => {
                self.loaded = Some(LoadedTab {
                    tab_id,
                    content,
                    dirty: false,
                    last_edit: None,
                });
                self.refresh_rename_buffers();
                self.save_status = SaveStatus::Saved;
            }
            Err(error) => self.set_error(error.to_string()),
        }
    }

    fn refresh_rename_buffers(&mut self) {
        if let Some(note) = self.current_note() {
            self.rename_note_title = note.title.clone();
        }
        if let Some(tab) = self.current_tab() {
            self.rename_tab_title = tab.title.clone();
            self.new_tab_title = self.new_tab_type.label().to_owned();
        }
    }

    fn mark_dirty(&mut self) {
        if let Some(loaded) = &mut self.loaded {
            loaded.dirty = true;
            loaded.last_edit = Some(Instant::now());
            self.save_status = SaveStatus::Unsaved;
        }
    }

    fn maybe_autosave(&mut self, context: &egui::Context) {
        let should_save = self
            .loaded
            .as_ref()
            .and_then(|loaded| loaded.last_edit.map(|last_edit| (loaded.dirty, last_edit)))
            .is_some_and(|(dirty, last_edit)| dirty && last_edit.elapsed() >= AUTOSAVE_AFTER);

        if should_save {
            self.save_current();
        } else if self.loaded.as_ref().is_some_and(|loaded| loaded.dirty) {
            context.request_repaint_after(AUTOSAVE_AFTER);
        }
    }

    fn flush_current(&mut self) {
        if self.loaded.as_ref().is_some_and(|loaded| loaded.dirty) {
            self.save_current();
        }
    }

    fn save_current(&mut self) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let Some(loaded) = &mut self.loaded else {
            return;
        };

        self.save_status = SaveStatus::Saving;
        match workspace.save_tab_content(&loaded.tab_id, &loaded.content) {
            Ok(()) => {
                loaded.dirty = false;
                loaded.last_edit = None;
                self.save_status = SaveStatus::Saved;
            }
            Err(error) => {
                self.save_status = SaveStatus::Error(error.to_string());
            }
        }
    }

    fn current_note(&self) -> Option<&crate::models::Note> {
        let workspace = self.workspace.as_ref()?;
        let selected = workspace.selected_note_id()?;
        workspace
            .config()
            .notes
            .iter()
            .find(|note| note.id == selected)
    }

    fn current_tab(&self) -> Option<&crate::models::Tab> {
        let workspace = self.workspace.as_ref()?;
        let selected = workspace.selected_tab_id()?;
        workspace
            .config()
            .notes
            .iter()
            .flat_map(|note| note.tabs.iter())
            .find(|tab| tab.id == selected)
    }

    fn set_error(&mut self, message: String) {
        self.message = Some(message.clone());
        self.save_status = SaveStatus::Error(message);
    }

    fn confirm_delete(&mut self) {
        let Some(action) = self.pending_confirm.take() else {
            return;
        };

        self.flush_current();
        match action {
            ConfirmAction::DeleteNote(note_id) => {
                if let Some(workspace) = &mut self.workspace {
                    match workspace.delete_note_config(&note_id) {
                        Ok(()) => {
                            self.loaded = None;
                            self.load_selected_content();
                        }
                        Err(error) => self.set_error(error.to_string()),
                    }
                }
            }
            ConfirmAction::DeleteTab(tab_id) => {
                if let Some(workspace) = &mut self.workspace {
                    match workspace.delete_tab_config(&tab_id) {
                        Ok(()) => {
                            self.loaded = None;
                            self.load_selected_content();
                        }
                        Err(error) => self.set_error(error.to_string()),
                    }
                }
            }
        }
    }
}

impl eframe::App for GimjiApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.maybe_autosave(context);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        self.render_sidebar(ui);
        self.render_main(ui);
        self.render_confirm(&context);
        self.render_message(&context);
    }

    fn on_exit(&mut self) {
        self.flush_current();
        self.recent.save();
    }
}

impl GimjiApp {
    fn render_sidebar(&mut self, root_ui: &mut egui::Ui) {
        egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(260.0)
            .show_inside(root_ui, |ui| {
                ui.heading("Notes");
                ui.horizontal(|ui| {
                    if ui.button("Open").clicked() {
                        self.open_workspace_dialog();
                    }
                    if ui.button("New").clicked() {
                        self.new_workspace_dialog();
                    }
                });

                if !self.recent.paths.is_empty() {
                    ui.separator();
                    ui.label("Recent Workspaces");
                    let recent_paths = self.recent.paths.clone();
                    for path in recent_paths {
                        let label = path.display().to_string();
                        if ui.button(shorten_path(&label)).clicked() {
                            self.open_workspace(path);
                        }
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.new_note_title);
                    if ui.button("+").clicked() {
                        self.add_note();
                    }
                });
                ui.text_edit_singleline(&mut self.note_filter);

                ui.separator();
                let notes: Vec<(String, String, bool)> = self
                    .workspace
                    .as_ref()
                    .map(|workspace| {
                        let selected = workspace.selected_note_id();
                        workspace
                            .config()
                            .notes
                            .iter()
                            .filter(|note| {
                                self.note_filter.trim().is_empty()
                                    || note
                                        .title
                                        .to_lowercase()
                                        .contains(&self.note_filter.to_lowercase())
                            })
                            .map(|note| {
                                (
                                    note.id.clone(),
                                    note.title.clone(),
                                    selected == Some(note.id.as_str()),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (id, title, selected) in notes {
                        if ui.selectable_label(selected, title).clicked() {
                            self.select_note(id);
                        }
                    }
                });
            });
    }

    fn render_main(&mut self, root_ui: &mut egui::Ui) {
        egui::CentralPanel::default().show_inside(root_ui, |ui| {
            let Some(workspace) = self.workspace.as_ref() else {
                ui.centered_and_justified(|ui| {
                    ui.label("Open or create a workspace folder.");
                });
                return;
            };

            let workspace_path = workspace.root().display().to_string();
            let selected_note = self.current_note().cloned();
            let selected_tab = self.current_tab().cloned();

            let Some(note) = selected_note else {
                ui.centered_and_justified(|ui| {
                    ui.label("Create a note from the sidebar.");
                });
                return;
            };

            ui.horizontal(|ui| {
                ui.heading(&note.title);
                ui.separator();
                ui.text_edit_singleline(&mut self.rename_note_title);
                if ui.button("Rename Note").clicked() {
                    self.rename_current_note();
                }
                if ui.button("Delete Note").clicked() {
                    self.pending_confirm = Some(ConfirmAction::DeleteNote(note.id.clone()));
                }
            });

            ui.separator();
            self.render_tab_row(ui, &note);

            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("new-tab-type")
                    .selected_text(self.new_tab_type.label())
                    .show_ui(ui, |ui| {
                        for tab_type in TabType::ALL {
                            ui.selectable_value(&mut self.new_tab_type, tab_type, tab_type.label());
                        }
                    });
                ui.text_edit_singleline(&mut self.new_tab_title);
                if ui.button("+ Tab").clicked() {
                    self.add_tab();
                }

                ui.separator();
                ui.text_edit_singleline(&mut self.rename_tab_title);
                if ui.button("Rename Tab").clicked() {
                    self.rename_current_tab();
                }
                if let Some(tab) = &selected_tab
                    && ui.button("Delete Tab").clicked()
                {
                    self.pending_confirm = Some(ConfirmAction::DeleteTab(tab.id.clone()));
                }
            });

            ui.separator();
            let tab_info = selected_tab
                .as_ref()
                .map(|tab| {
                    format!(
                        "{} | {} | {} | {}",
                        workspace_path,
                        tab.tab_type.as_str(),
                        tab.file_name,
                        self.save_status.label()
                    )
                })
                .unwrap_or_else(|| {
                    format!("{workspace_path} | no tab | {}", self.save_status.label())
                });
            ui.label(egui::RichText::new(tab_info).small());
            ui.separator();

            if selected_tab.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label("Add a tab to this note.");
                });
                return;
            }

            self.render_content(ui);
        });
    }

    fn render_tab_row(&mut self, ui: &mut egui::Ui, note: &crate::models::Note) {
        egui::ScrollArea::horizontal()
            .id_salt("tab-row")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let selected_tab = self
                        .workspace
                        .as_ref()
                        .and_then(|workspace| workspace.selected_tab_id())
                        .map(str::to_owned);
                    for tab in &note.tabs {
                        let label = format!("{} ({})", tab.title, tab.tab_type.as_str());
                        if ui
                            .selectable_label(
                                selected_tab.as_deref() == Some(tab.id.as_str()),
                                label,
                            )
                            .clicked()
                        {
                            self.select_tab(tab.id.clone());
                        }
                    }
                });
            });
    }

    fn render_content(&mut self, ui: &mut egui::Ui) {
        let Some(loaded) = &mut self.loaded else {
            ui.centered_and_justified(|ui| {
                ui.label("No content loaded.");
            });
            return;
        };

        let dirty = match &mut loaded.content {
            TabContent::Markdown(markdown) => render_markdown(ui, markdown),
            TabContent::Kanban(board) => render_kanban(ui, board),
            TabContent::Todo(todo) => render_todo(ui, todo),
            TabContent::Calendar(calendar) => render_calendar(ui, calendar),
        };

        if dirty {
            self.mark_dirty();
        }
    }

    fn render_confirm(&mut self, context: &egui::Context) {
        let Some(action) = &self.pending_confirm else {
            return;
        };

        let message = match action {
            ConfirmAction::DeleteNote(_) => {
                "Delete this note from config? Content files stay on disk."
            }
            ConfirmAction::DeleteTab(_) => {
                "Delete this tab from config? Content file stays on disk."
            }
        };

        egui::Window::new("Confirm Delete")
            .collapsible(false)
            .resizable(false)
            .show(context, |ui| {
                ui.label(message);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.pending_confirm = None;
                    }
                    if ui.button("Delete").clicked() {
                        self.confirm_delete();
                    }
                });
            });
    }

    fn render_message(&mut self, context: &egui::Context) {
        let Some(message) = self.message.clone() else {
            return;
        };

        egui::Window::new("Status")
            .collapsible(false)
            .resizable(true)
            .show(context, |ui| {
                ui.label(message);
                if ui.button("OK").clicked() {
                    self.message = None;
                }
            });
    }
}

#[derive(Debug)]
struct LoadedTab {
    tab_id: String,
    content: TabContent,
    dirty: bool,
    last_edit: Option<Instant>,
}

#[derive(Debug, Clone)]
enum SaveStatus {
    Idle,
    Unsaved,
    Saving,
    Saved,
    Error(String),
}

impl SaveStatus {
    fn label(&self) -> String {
        match self {
            Self::Idle => "Idle".to_owned(),
            Self::Unsaved => "Unsaved changes".to_owned(),
            Self::Saving => "Saving".to_owned(),
            Self::Saved => "Saved".to_owned(),
            Self::Error(message) => format!("Error: {message}"),
        }
    }
}

#[derive(Debug, Clone)]
enum ConfirmAction {
    DeleteNote(String),
    DeleteTab(String),
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RecentWorkspaces {
    paths: Vec<PathBuf>,
}

impl RecentWorkspaces {
    fn load() -> Self {
        let Some(path) = recent_workspaces_path() else {
            return Self::default();
        };
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    fn add(&mut self, path: PathBuf) {
        self.paths.retain(|recent| recent != &path);
        self.paths.insert(0, path);
        self.paths.truncate(8);
        self.save();
    }

    fn save(&self) {
        let Some(path) = recent_workspaces_path() else {
            return;
        };
        if let Some(parent) = path.parent()
            && fs::create_dir_all(parent).is_err()
        {
            return;
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, text);
        }
    }
}

fn recent_workspaces_path() -> Option<PathBuf> {
    ProjectDirs::from("dev", "mod", "Gimji")
        .map(|project_dirs| project_dirs.config_dir().join("recent_workspaces.json"))
}

fn render_markdown(ui: &mut egui::Ui, markdown: &mut String) -> bool {
    ui.add_sized(
        ui.available_size(),
        egui::TextEdit::multiline(markdown)
            .font(egui::TextStyle::Monospace)
            .desired_rows(24),
    )
    .changed()
}

fn render_kanban(ui: &mut egui::Ui, board: &mut KanbanBoard) -> bool {
    let mut dirty = false;
    let mut action = None;

    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for column_index in 0..board.columns.len() {
                ui.group(|ui| {
                    ui.set_width(280.0);
                    ui.horizontal(|ui| {
                        ui.heading(&board.columns[column_index].title);
                        if ui.button("+ Card").clicked() {
                            action = Some(KanbanAction::AddCard(column_index));
                        }
                    });

                    let card_count = board.columns[column_index].cards.len();
                    for card_index in 0..card_count {
                        ui.separator();
                        let card = &mut board.columns[column_index].cards[card_index];
                        if ui
                            .add(
                                egui::TextEdit::multiline(&mut card.text)
                                    .desired_width(250.0)
                                    .desired_rows(3),
                            )
                            .changed()
                        {
                            card.touch();
                            dirty = true;
                        }
                        ui.horizontal(|ui| {
                            if ui.button("Left").clicked() {
                                action = Some(KanbanAction::MoveColumn {
                                    column_index,
                                    card_index,
                                    delta: -1,
                                });
                            }
                            if ui.button("Right").clicked() {
                                action = Some(KanbanAction::MoveColumn {
                                    column_index,
                                    card_index,
                                    delta: 1,
                                });
                            }
                            if ui.button("Up").clicked() {
                                action = Some(KanbanAction::MoveRow {
                                    column_index,
                                    card_index,
                                    delta: -1,
                                });
                            }
                            if ui.button("Down").clicked() {
                                action = Some(KanbanAction::MoveRow {
                                    column_index,
                                    card_index,
                                    delta: 1,
                                });
                            }
                            if ui.button("Delete").clicked() {
                                action = Some(KanbanAction::DeleteCard {
                                    column_index,
                                    card_index,
                                });
                            }
                        });
                    }
                });
            }
        });
    });

    if let Some(action) = action {
        apply_kanban_action(board, action);
        dirty = true;
    }

    dirty
}

fn render_todo(ui: &mut egui::Ui, todo: &mut TodoList) -> bool {
    let mut dirty = false;
    let mut delete_index = None;

    if ui.button("+ Todo").clicked() {
        todo.items.push(TodoItem::new("New todo"));
        dirty = true;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (index, item) in todo.items.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                if ui.checkbox(&mut item.done, "").changed() {
                    item.touch();
                    dirty = true;
                }
                if ui.text_edit_singleline(&mut item.text).changed() {
                    item.touch();
                    dirty = true;
                }
                if ui.button("Delete").clicked() {
                    delete_index = Some(index);
                }
            });
        }
    });

    if let Some(index) = delete_index {
        todo.items.remove(index);
        dirty = true;
    }

    dirty
}

fn render_calendar(ui: &mut egui::Ui, calendar: &mut CalendarData) -> bool {
    let mut dirty = false;
    let mut delete_index = None;

    if ui.button("+ Event").clicked() {
        calendar.events.push(CalendarEvent::new(
            Local::now().format("%Y-%m-%d").to_string(),
            "New event",
            "",
        ));
        dirty = true;
    }

    calendar.events.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then(left.title.cmp(&right.title))
    });

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (index, event) in calendar.events.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Date");
                    if ui.text_edit_singleline(&mut event.date).changed() {
                        event.touch();
                        dirty = true;
                    }
                    ui.label("Title");
                    if ui.text_edit_singleline(&mut event.title).changed() {
                        event.touch();
                        dirty = true;
                    }
                    if ui.button("Delete").clicked() {
                        delete_index = Some(index);
                    }
                });
                if ui
                    .add(
                        egui::TextEdit::multiline(&mut event.description)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    event.touch();
                    dirty = true;
                }
            });
        }
    });

    if let Some(index) = delete_index {
        calendar.events.remove(index);
        dirty = true;
    }

    dirty
}

#[derive(Debug, Clone, Copy)]
enum KanbanAction {
    AddCard(usize),
    DeleteCard {
        column_index: usize,
        card_index: usize,
    },
    MoveColumn {
        column_index: usize,
        card_index: usize,
        delta: isize,
    },
    MoveRow {
        column_index: usize,
        card_index: usize,
        delta: isize,
    },
}

fn apply_kanban_action(board: &mut KanbanBoard, action: KanbanAction) {
    match action {
        KanbanAction::AddCard(column_index) => {
            if let Some(column) = board.columns.get_mut(column_index) {
                column.cards.push(KanbanCard::new("New card"));
            }
        }
        KanbanAction::DeleteCard {
            column_index,
            card_index,
        } => {
            if let Some(column) = board.columns.get_mut(column_index)
                && card_index < column.cards.len()
            {
                column.cards.remove(card_index);
            }
        }
        KanbanAction::MoveColumn {
            column_index,
            card_index,
            delta,
        } => {
            let destination = column_index as isize + delta;
            if destination < 0 || destination >= board.columns.len() as isize {
                return;
            }
            if card_index >= board.columns[column_index].cards.len() {
                return;
            }
            let card = board.columns[column_index].cards.remove(card_index);
            board.columns[destination as usize].cards.push(card);
        }
        KanbanAction::MoveRow {
            column_index,
            card_index,
            delta,
        } => {
            let Some(column) = board.columns.get_mut(column_index) else {
                return;
            };
            let destination = card_index as isize + delta;
            if destination < 0 || destination >= column.cards.len() as isize {
                return;
            }
            column.cards.swap(card_index, destination as usize);
        }
    }
}

fn shorten_path(path: &str) -> String {
    const MAX_LEN: usize = 38;
    if path.chars().count() <= MAX_LEN {
        return path.to_owned();
    }

    let tail: String = path
        .chars()
        .rev()
        .take(MAX_LEN - 3)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("...{tail}")
}
