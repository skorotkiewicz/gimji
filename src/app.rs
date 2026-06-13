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
const APP_BG: egui::Color32 = egui::Color32::from_rgb(18, 19, 21);
const SIDEBAR_BG: egui::Color32 = egui::Color32::from_rgb(24, 26, 29);
const SURFACE_BG: egui::Color32 = egui::Color32::from_rgb(31, 34, 38);
const SURFACE_HOVER: egui::Color32 = egui::Color32::from_rgb(39, 43, 48);
const ACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(54, 70, 92);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(84, 162, 132);
const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(154, 163, 175);
const STROKE: egui::Color32 = egui::Color32::from_rgb(52, 56, 62);
const NOTE_HEADER_ACTION_HEIGHT: f32 = 36.0;
const KANBAN_COLUMN_WIDTH: f32 = 280.0;
const KANBAN_CARD_TEXT_WIDTH: f32 = 250.0;
const KANBAN_CARD_TEXT_HEIGHT: f32 = 76.0;

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
    fn new(creation_context: &eframe::CreationContext<'_>) -> Self {
        configure_theme(&creation_context.egui_ctx);

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

    fn handle_shortcuts(&mut self, context: &egui::Context) {
        if context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save_current();
        }

        if context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::N)) {
            self.add_note();
        }

        if context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::T)) {
            self.add_tab();
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
        self.handle_shortcuts(context);
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
            .default_size(300.0)
            .frame(
                egui::Frame::new()
                    .fill(SIDEBAR_BG)
                    .inner_margin(egui::Margin::same(14)),
            )
            .show_inside(root_ui, |ui| {
                ui.heading("Gimji");
                ui.label(
                    egui::RichText::new("Workspace notes")
                        .small()
                        .color(TEXT_MUTED),
                );
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 8.0) / 2.0;
                    if ui
                        .add_sized([width, 30.0], egui::Button::new("Open"))
                        .on_hover_text("Open workspace")
                        .clicked()
                    {
                        self.open_workspace_dialog();
                    }
                    if ui
                        .add_sized([width, 30.0], egui::Button::new("New"))
                        .on_hover_text("Create workspace")
                        .clicked()
                    {
                        self.new_workspace_dialog();
                    }
                });

                if !self.recent.paths.is_empty() {
                    ui.add_space(8.0);
                    section_label(ui, "Recent");
                    let recent_paths = self.recent.paths.clone();
                    for path in recent_paths {
                        let label = path.display().to_string();
                        if ui
                            .add_sized(
                                [ui.available_width(), 28.0],
                                egui::Button::new(shorten_path(&label)).fill(SURFACE_BG),
                            )
                            .on_hover_text(label)
                            .clicked()
                        {
                            self.open_workspace(path);
                        }
                    }
                }

                ui.add_space(8.0);
                section_label(ui, "New Note");
                ui.horizontal(|ui| {
                    let add_width = 38.0;
                    ui.add_sized(
                        [ui.available_width() - add_width - 8.0, 30.0],
                        egui::TextEdit::singleline(&mut self.new_note_title).hint_text("Title"),
                    );
                    if ui
                        .add_sized([add_width, 30.0], egui::Button::new("+"))
                        .on_hover_text("Add note")
                        .clicked()
                    {
                        self.add_note();
                    }
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.note_filter)
                        .hint_text("Search notes")
                        .desired_width(f32::INFINITY),
                );

                ui.add_space(8.0);
                section_label(ui, "Notes");
                let notes: Vec<(String, String, usize, bool)> = self
                    .workspace
                    .as_ref()
                    .map(|workspace| {
                        let selected = workspace.selected_note_id();
                        workspace
                            .config()
                            .notes
                            .iter()
                            .filter(|note| note_matches_filter(&note.title, &self.note_filter))
                            .map(|note| {
                                (
                                    note.id.clone(),
                                    note.title.clone(),
                                    note.tabs.len(),
                                    selected == Some(note.id.as_str()),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (id, title, tab_count, selected) in notes {
                        let label = format!("{title}  {tab_count} tabs");
                        let fill = if selected { ACTIVE_BG } else { SURFACE_BG };
                        if ui
                            .add_sized(
                                [ui.available_width(), 34.0],
                                egui::Button::new(label).selected(selected).fill(fill),
                            )
                            .clicked()
                        {
                            self.select_note(id);
                        }
                    }
                });
            });
    }

    fn render_main(&mut self, root_ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(APP_BG)
                    .inner_margin(egui::Margin::same(16)),
            )
            .show_inside(root_ui, |ui| {
                let Some(workspace) = self.workspace.as_ref() else {
                    render_empty_state(ui, "No workspace", "Open or create a folder to begin.");
                    return;
                };

                let workspace_path = workspace.root().display().to_string();
                let selected_note = self.current_note().cloned();
                let selected_tab = self.current_tab().cloned();

                let Some(note) = selected_note else {
                    render_empty_state(ui, "No note selected", "Create a note from the sidebar.");
                    return;
                };

                panel_frame(SURFACE_BG).show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.heading(&note.title);
                            ui.label(
                                egui::RichText::new(format!("{} tabs", note.tabs.len()))
                                    .small()
                                    .color(TEXT_MUTED),
                            );
                        });
                        ui.allocate_ui_with_layout(
                            note_header_action_area_size(ui.available_width()),
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .button("Delete Note")
                                    .on_hover_text("Remove note metadata only")
                                    .clicked()
                                {
                                    self.pending_confirm =
                                        Some(ConfirmAction::DeleteNote(note.id.clone()));
                                }
                                if ui.button("Rename Note").clicked() {
                                    self.rename_current_note();
                                }
                                ui.add_sized(
                                    [220.0, 28.0],
                                    egui::TextEdit::singleline(&mut self.rename_note_title)
                                        .hint_text("Note title"),
                                );
                            },
                        );
                    });
                });

                ui.add_space(8.0);
                self.render_tab_row(ui, &note);

                ui.add_space(4.0);
                let [create_title, selected_title] = tab_action_section_titles();
                panel_frame(egui::Color32::from_rgb(25, 27, 30)).show(ui, |ui| {
                    section_label(ui, create_title);
                    ui.horizontal_wrapped(|ui| {
                        egui::ComboBox::from_id_salt("new-tab-type")
                            .selected_text(self.new_tab_type.label())
                            .show_ui(ui, |ui| {
                                for tab_type in TabType::ALL {
                                    ui.selectable_value(
                                        &mut self.new_tab_type,
                                        tab_type,
                                        tab_type.label(),
                                    );
                                }
                            });
                        ui.add_sized(
                            [180.0, 28.0],
                            egui::TextEdit::singleline(&mut self.new_tab_title)
                                .hint_text("Tab title"),
                        );
                        if ui.button("+ Tab").clicked() {
                            self.add_tab();
                        }
                    });
                });

                ui.add_space(4.0);
                panel_frame(egui::Color32::from_rgb(25, 27, 30)).show(ui, |ui| {
                    section_label(ui, selected_title);
                    ui.horizontal_wrapped(|ui| {
                        ui.add_sized(
                            [180.0, 28.0],
                            egui::TextEdit::singleline(&mut self.rename_tab_title)
                                .hint_text("Selected tab"),
                        );
                        if ui.button("Rename Tab").clicked() {
                            self.rename_current_tab();
                        }
                        if let Some(tab) = &selected_tab
                            && ui
                                .button("Delete Tab")
                                .on_hover_text("Remove tab metadata only")
                                .clicked()
                        {
                            self.pending_confirm = Some(ConfirmAction::DeleteTab(tab.id.clone()));
                        }
                    });
                });

                ui.add_space(8.0);
                render_status_strip(
                    ui,
                    &workspace_path,
                    selected_tab.as_ref(),
                    &self.save_status,
                );
                ui.add_space(8.0);

                if selected_tab.is_none() {
                    render_empty_state(ui, "No tab selected", "Add a tab to this note.");
                    return;
                }

                self.render_content(ui);
            });
    }

    fn render_tab_row(&mut self, ui: &mut egui::Ui, note: &crate::models::Note) {
        panel_frame(egui::Color32::from_rgb(25, 27, 30)).show(ui, |ui| {
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
                            let selected = selected_tab.as_deref() == Some(tab.id.as_str());
                            let label = format!("{}  {}", tab.title, tab.tab_type.as_str());
                            let fill = if selected { ACTIVE_BG } else { SURFACE_BG };
                            if ui
                                .add(
                                    egui::Button::new(label)
                                        .selected(selected)
                                        .fill(fill)
                                        .min_size(egui::vec2(120.0, 30.0)),
                                )
                                .clicked()
                            {
                                self.select_tab(tab.id.clone());
                            }
                        }
                    });
                });
        });
    }

    fn render_content(&mut self, ui: &mut egui::Ui) {
        let Some(loaded) = &mut self.loaded else {
            render_empty_state(ui, "No content", "Select a tab to load its content.");
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

fn configure_theme(context: &egui::Context) {
    let mut style = (*context.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(12);
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = APP_BG;
    style.visuals.window_fill = SURFACE_BG;
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(14, 15, 17);
    style.visuals.faint_bg_color = SURFACE_BG;
    style.visuals.widgets.noninteractive.bg_fill = SURFACE_BG;
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(37, 40, 44);
    style.visuals.widgets.hovered.bg_fill = SURFACE_HOVER;
    style.visuals.widgets.active.bg_fill = ACTIVE_BG;
    style.visuals.selection.bg_fill = ACTIVE_BG;
    style.visuals.hyperlink_color = ACCENT;
    context.set_global_style(style);
}

fn note_matches_filter(title: &str, filter: &str) -> bool {
    let filter = filter.trim();
    filter.is_empty() || title.to_lowercase().contains(&filter.to_lowercase())
}

fn note_header_action_area_size(width: f32) -> egui::Vec2 {
    egui::vec2(width.max(0.0), NOTE_HEADER_ACTION_HEIGHT)
}

fn kanban_column_header_action_area_size(width: f32) -> egui::Vec2 {
    egui::vec2(width.max(0.0), NOTE_HEADER_ACTION_HEIGHT)
}

fn kanban_column_area_size() -> egui::Vec2 {
    egui::vec2(KANBAN_COLUMN_WIDTH, 0.0)
}

fn kanban_card_text_area_size() -> egui::Vec2 {
    egui::vec2(KANBAN_CARD_TEXT_WIDTH, KANBAN_CARD_TEXT_HEIGHT)
}

fn tab_action_section_titles() -> [&'static str; 2] {
    ["Create Tab", "Selected Tab"]
}

fn panel_frame(fill: egui::Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .inner_margin(egui::Margin::same(12))
        .corner_radius(8)
        .stroke(egui::Stroke::new(1.0, STROKE))
}

fn status_color(status: &SaveStatus) -> egui::Color32 {
    match status {
        SaveStatus::Idle => TEXT_MUTED,
        SaveStatus::Unsaved => egui::Color32::from_rgb(230, 178, 96),
        SaveStatus::Saving => egui::Color32::from_rgb(102, 171, 238),
        SaveStatus::Saved => ACCENT,
        SaveStatus::Error(_) => egui::Color32::from_rgb(232, 116, 116),
    }
}

fn section_label(ui: &mut egui::Ui, label: &str) {
    ui.label(
        egui::RichText::new(label)
            .small()
            .strong()
            .color(TEXT_MUTED),
    );
}

fn render_empty_state(ui: &mut egui::Ui, title: &str, detail: &str) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(12.0);
            ui.heading(title);
            ui.label(egui::RichText::new(detail).color(TEXT_MUTED));
        });
    });
}

fn render_status_strip(
    ui: &mut egui::Ui,
    workspace_path: &str,
    selected_tab: Option<&crate::models::Tab>,
    save_status: &SaveStatus,
) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(20, 22, 25))
        .inner_margin(egui::Margin::symmetric(10, 7))
        .corner_radius(6)
        .stroke(egui::Stroke::new(1.0, STROKE))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(save_status.label())
                        .small()
                        .strong()
                        .color(status_color(save_status)),
                );
                ui.separator();
                if let Some(tab) = selected_tab {
                    ui.label(
                        egui::RichText::new(tab.tab_type.as_str())
                            .small()
                            .color(TEXT_MUTED),
                    );
                    ui.separator();
                    ui.label(
                        egui::RichText::new(&tab.file_name)
                            .small()
                            .color(TEXT_MUTED),
                    );
                } else {
                    ui.label(egui::RichText::new("no tab").small().color(TEXT_MUTED));
                }
                ui.separator();
                ui.label(
                    egui::RichText::new(workspace_path)
                        .small()
                        .color(TEXT_MUTED),
                );
            });
        });
}

fn render_markdown(ui: &mut egui::Ui, markdown: &mut String) -> bool {
    panel_frame(SURFACE_BG)
        .show(ui, |ui| {
            ui.add_sized(
                ui.available_size(),
                egui::TextEdit::multiline(markdown)
                    .font(egui::TextStyle::Monospace)
                    .hint_text("Write markdown...")
                    .desired_width(f32::INFINITY)
                    .desired_rows(24),
            )
            .changed()
        })
        .inner
}

fn render_kanban(ui: &mut egui::Ui, board: &mut KanbanBoard) -> bool {
    let mut dirty = false;
    let mut action = None;

    panel_frame(SURFACE_BG).show(ui, |ui| {
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for column_index in 0..board.columns.len() {
                    ui.allocate_ui_with_layout(
                        kanban_column_area_size(),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::Frame::new()
                                .fill(egui::Color32::from_rgb(25, 28, 32))
                                .inner_margin(egui::Margin::same(10))
                                .corner_radius(8)
                                .stroke(egui::Stroke::new(1.0, STROKE))
                                .show(ui, |ui| {
                                    ui.set_width(KANBAN_COLUMN_WIDTH);
                                    ui.horizontal(|ui| {
                                        ui.heading(&board.columns[column_index].title);
                                        ui.allocate_ui_with_layout(
                                            kanban_column_header_action_area_size(
                                                ui.available_width(),
                                            ),
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .button("+ Card")
                                                    .on_hover_text("Add card")
                                                    .clicked()
                                                {
                                                    action =
                                                        Some(KanbanAction::AddCard(column_index));
                                                }
                                            },
                                        );
                                    });

                                    let card_count = board.columns[column_index].cards.len();
                                    if card_count == 0 {
                                        ui.add_space(8.0);
                                        ui.label(
                                            egui::RichText::new("No cards")
                                                .small()
                                                .color(TEXT_MUTED),
                                        );
                                    }

                                    for card_index in 0..card_count {
                                        ui.add_space(8.0);
                                        egui::Frame::new()
                                            .fill(SURFACE_BG)
                                            .inner_margin(egui::Margin::same(8))
                                            .corner_radius(6)
                                            .stroke(egui::Stroke::new(1.0, STROKE))
                                            .show(ui, |ui| {
                                                ui.set_width(KANBAN_CARD_TEXT_WIDTH);
                                                let card = &mut board.columns[column_index].cards
                                                    [card_index];
                                                if ui
                                                    .add_sized(
                                                        kanban_card_text_area_size(),
                                                        egui::TextEdit::multiline(&mut card.text)
                                                            .desired_rows(3),
                                                    )
                                                    .changed()
                                                {
                                                    card.touch();
                                                    dirty = true;
                                                }
                                                ui.horizontal(|ui| {
                                                    if ui
                                                        .small_button("<")
                                                        .on_hover_text("Move left")
                                                        .clicked()
                                                    {
                                                        action = Some(KanbanAction::MoveColumn {
                                                            column_index,
                                                            card_index,
                                                            delta: -1,
                                                        });
                                                    }
                                                    if ui
                                                        .small_button(">")
                                                        .on_hover_text("Move right")
                                                        .clicked()
                                                    {
                                                        action = Some(KanbanAction::MoveColumn {
                                                            column_index,
                                                            card_index,
                                                            delta: 1,
                                                        });
                                                    }
                                                    if ui
                                                        .small_button("Up")
                                                        .on_hover_text("Move up")
                                                        .clicked()
                                                    {
                                                        action = Some(KanbanAction::MoveRow {
                                                            column_index,
                                                            card_index,
                                                            delta: -1,
                                                        });
                                                    }
                                                    if ui
                                                        .small_button("Dn")
                                                        .on_hover_text("Move down")
                                                        .clicked()
                                                    {
                                                        action = Some(KanbanAction::MoveRow {
                                                            column_index,
                                                            card_index,
                                                            delta: 1,
                                                        });
                                                    }
                                                    if ui
                                                        .small_button("Del")
                                                        .on_hover_text("Delete card")
                                                        .clicked()
                                                    {
                                                        action = Some(KanbanAction::DeleteCard {
                                                            column_index,
                                                            card_index,
                                                        });
                                                    }
                                                });
                                            });
                                    }
                                });
                        },
                    );
                }
            });
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

    panel_frame(SURFACE_BG).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Tasks");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("+ Todo").clicked() {
                    todo.items.push(TodoItem::new("New todo"));
                    dirty = true;
                }
            });
        });

        if todo.items.is_empty() {
            ui.add_space(12.0);
            ui.label(egui::RichText::new("No tasks").color(TEXT_MUTED));
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (index, item) in todo.items.iter_mut().enumerate() {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(25, 28, 32))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .corner_radius(6)
                    .stroke(egui::Stroke::new(1.0, STROKE))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut item.done, "").changed() {
                                item.touch();
                                dirty = true;
                            }
                            if ui
                                .add_sized(
                                    [ui.available_width() - 72.0, 28.0],
                                    egui::TextEdit::singleline(&mut item.text),
                                )
                                .changed()
                            {
                                item.touch();
                                dirty = true;
                            }
                            if ui
                                .small_button("Del")
                                .on_hover_text("Delete todo")
                                .clicked()
                            {
                                delete_index = Some(index);
                            }
                        });
                    });
                ui.add_space(6.0);
            }
        });
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

    calendar.events.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then(left.title.cmp(&right.title))
    });

    panel_frame(SURFACE_BG).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Calendar");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("+ Event").clicked() {
                    calendar.events.push(CalendarEvent::new(
                        Local::now().format("%Y-%m-%d").to_string(),
                        "New event",
                        "",
                    ));
                    dirty = true;
                }
            });
        });

        if calendar.events.is_empty() {
            ui.add_space(12.0);
            ui.label(egui::RichText::new("No events").color(TEXT_MUTED));
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (index, event) in calendar.events.iter_mut().enumerate() {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(25, 28, 32))
                    .inner_margin(egui::Margin::same(10))
                    .corner_radius(6)
                    .stroke(egui::Stroke::new(1.0, STROKE))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Date").small().color(TEXT_MUTED));
                            if ui
                                .add_sized(
                                    [112.0, 28.0],
                                    egui::TextEdit::singleline(&mut event.date),
                                )
                                .changed()
                            {
                                event.touch();
                                dirty = true;
                            }
                            ui.label(egui::RichText::new("Title").small().color(TEXT_MUTED));
                            if ui
                                .add_sized(
                                    [ui.available_width() - 72.0, 28.0],
                                    egui::TextEdit::singleline(&mut event.title),
                                )
                                .changed()
                            {
                                event.touch();
                                dirty = true;
                            }
                            if ui
                                .small_button("Del")
                                .on_hover_text("Delete event")
                                .clicked()
                            {
                                delete_index = Some(index);
                            }
                        });
                        if ui
                            .add(
                                egui::TextEdit::multiline(&mut event.description)
                                    .hint_text("Description")
                                    .desired_rows(2)
                                    .desired_width(f32::INFINITY),
                            )
                            .changed()
                        {
                            event.touch();
                            dirty = true;
                        }
                    });
                ui.add_space(6.0);
            }
        });
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

#[cfg(test)]
mod tests {
    use super::{
        KANBAN_CARD_TEXT_HEIGHT, KANBAN_CARD_TEXT_WIDTH, KANBAN_COLUMN_WIDTH,
        NOTE_HEADER_ACTION_HEIGHT, kanban_card_text_area_size, kanban_column_area_size,
        kanban_column_header_action_area_size, note_header_action_area_size, note_matches_filter,
        tab_action_section_titles,
    };

    #[test]
    fn note_filter_ignores_case_and_surrounding_whitespace() {
        assert!(note_matches_filter("Project Notes", " project "));
        assert!(note_matches_filter("Project Notes", "NOTES"));
        assert!(note_matches_filter("Project Notes", ""));
        assert!(!note_matches_filter("Project Notes", "archive"));
    }

    #[test]
    fn note_header_actions_use_compact_height() {
        let size = note_header_action_area_size(640.0);

        assert_eq!(size.x, 640.0);
        assert_eq!(size.y, NOTE_HEADER_ACTION_HEIGHT);
    }

    #[test]
    fn kanban_column_header_actions_use_compact_height() {
        let size = kanban_column_header_action_area_size(120.0);

        assert_eq!(size.x, 120.0);
        assert_eq!(size.y, NOTE_HEADER_ACTION_HEIGHT);
    }

    #[test]
    fn kanban_column_uses_stable_top_down_area() {
        let size = kanban_column_area_size();

        assert_eq!(size.x, KANBAN_COLUMN_WIDTH);
        assert_eq!(size.y, 0.0);
    }

    #[test]
    fn kanban_card_text_editor_uses_readable_width() {
        let size = kanban_card_text_area_size();

        assert_eq!(size.x, KANBAN_CARD_TEXT_WIDTH);
        assert_eq!(size.y, KANBAN_CARD_TEXT_HEIGHT);
    }

    #[test]
    fn tab_actions_are_split_between_create_and_selected_tab_sections() {
        let sections = tab_action_section_titles();

        assert_eq!(sections, ["Create Tab", "Selected Tab"]);
    }
}
