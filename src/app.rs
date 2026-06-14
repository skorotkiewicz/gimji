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
#[cfg(test)]
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
    renaming_tab: bool,
    rename_tab_id: Option<String>,
    save_status: SaveStatus,
    pending_confirm: Option<ConfirmAction>,
    message: Option<String>,
    // UI State
    editing_note_title: bool,
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
            renaming_tab: false,
            rename_tab_id: None,
            save_status: SaveStatus::Idle,
            pending_confirm: None,
            message: None,
            editing_note_title: false,
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
        self.renaming_tab = false;
        self.rename_tab_id = None;
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
        self.renaming_tab = false;
        self.rename_tab_id = None;
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
                    self.renaming_tab = false;
                    self.rename_tab_id = None;
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
        } else {
            self.editing_note_title = false;
        }
    }

    fn rename_current_tab(&mut self) {
        let title = self.rename_tab_title.trim().to_owned();
        if title.is_empty() {
            return;
        }

        let Some(tab_id) = self.rename_tab_id.clone().or_else(|| {
            self.workspace
                .as_ref()
                .and_then(|workspace| workspace.selected_tab_id().map(str::to_owned))
        }) else {
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
        self.renaming_tab = false;
        self.rename_tab_id = None;
        if let Some(workspace) = &mut self.workspace {
            match workspace.select_note(&note_id) {
                Ok(()) => {
                    self.loaded = None;
                    self.editing_note_title = false;
                    self.load_selected_content();
                }
                Err(error) => self.set_error(error.to_string()),
            }
        }
    }

    fn select_tab(&mut self, tab_id: String) {
        self.flush_current();
        self.renaming_tab = false;
        self.rename_tab_id = None;
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
                            self.renaming_tab = false;
                            self.rename_tab_id = None;
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
                            self.renaming_tab = false;
                            self.rename_tab_id = None;
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
            .default_size(200.0)
            .size_range(180.0..=200.0)
            .frame(
                egui::Frame::new()
                    .fill(SIDEBAR_BG)
                    .inner_margin(egui::Margin::symmetric(14, 14)),
            )
            .show_inside(root_ui, |ui| {
                ui.vertical(|ui| {
                    ui.heading("Gimji");
                    ui.label(egui::RichText::new("Workspace").small().color(TEXT_MUTED));
                    ui.add_space(10.0);

                    // Workspace Actions
                    ui.horizontal(|ui| {
                        let width = (ui.available_width() - 6.0) / 2.0;
                        if ui
                            .add_sized([width, 28.0], egui::Button::new("Open"))
                            .clicked()
                        {
                            self.open_workspace_dialog();
                        }
                        if ui
                            .add_sized([width, 28.0], egui::Button::new("New"))
                            .clicked()
                        {
                            self.new_workspace_dialog();
                        }
                    });

                    // Recent
                    if !self.recent.paths.is_empty() {
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);
                        section_label(ui, "Recent");
                        for path in self.recent.paths.clone() {
                            let label = path.display().to_string();
                            if ui
                                .add_sized(
                                    [ui.available_width(), 24.0],
                                    egui::Button::new(shorten_path(&label))
                                        .fill(egui::Color32::TRANSPARENT)
                                        .small(),
                                )
                                .on_hover_text(label)
                                .clicked()
                            {
                                self.open_workspace(path);
                            }
                        }
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Add Note
                    section_label(ui, "Notes");
                    ui.horizontal(|ui| {
                        let add_width = 34.0;
                        ui.add_sized(
                            [ui.available_width() - add_width - 6.0, 26.0],
                            egui::TextEdit::singleline(&mut self.new_note_title)
                                .hint_text("New note title"),
                        );
                        if ui
                            .add_sized([add_width, 26.0], egui::Button::new("+").small())
                            .clicked()
                        {
                            self.add_note();
                        }
                    });

                    // Search
                    ui.add(
                        egui::TextEdit::singleline(&mut self.note_filter)
                            .hint_text("Filter")
                            .desired_width(f32::INFINITY)
                            .margin(egui::Vec2::new(4.0, 4.0)),
                    );

                    ui.add_space(6.0);

                    // Notes List
                    let notes: Vec<(String, String, bool)> = self
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
                                        selected == Some(note.id.as_str()),
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (id, title, selected) in notes {
                                let bg = if selected {
                                    ACTIVE_BG
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                if ui
                                    .add_sized(
                                        [ui.available_width(), 30.0],
                                        egui::Button::new(egui::RichText::new(title).size(14.0))
                                            .fill(bg)
                                            .frame(false)
                                            .selected(selected),
                                    )
                                    .clicked()
                                {
                                    self.select_note(id);
                                }
                            }
                        });
                });
            });
    }

    fn render_main(&mut self, root_ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(APP_BG)
                    .inner_margin(egui::Margin::symmetric(20, 12)),
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

                // 1. Note Header
                self.render_note_header(ui, &note);

                ui.add_space(8.0);

                // 2. Tab Row
                self.render_tab_row(ui, &note);

                ui.add_space(4.0);

                // 3. Tab Toolbar
                self.render_tab_toolbar(ui);

                ui.add_space(8.0);

                // 4. Status Strip
                render_status_strip(
                    ui,
                    &workspace_path,
                    selected_tab.as_ref(),
                    &self.save_status,
                );

                ui.add_space(8.0);

                // 5. Content Area
                if selected_tab.is_none() {
                    render_empty_state(
                        ui,
                        "No tab selected",
                        "Add a tab to this note to start editing.",
                    );
                } else {
                    self.render_content(ui);
                }
            });
    }

    fn render_note_header(&mut self, ui: &mut egui::Ui, note: &crate::models::Note) {
        ui.horizontal(|ui| {
            if self.editing_note_title {
                ui.add_sized(
                    [ui.available_width() - 80.0, 28.0],
                    egui::TextEdit::singleline(&mut self.rename_note_title),
                );
                if ui.button("Save").clicked() {
                    self.rename_current_note();
                }
                if ui.button("Cancel").clicked() {
                    self.editing_note_title = false;
                    self.refresh_rename_buffers();
                }
            } else {
                ui.heading(&note.title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button("Delete Note")
                        .on_hover_text("Remove note metadata only")
                        .clicked()
                    {
                        self.pending_confirm = Some(ConfirmAction::DeleteNote(note.id.clone()));
                    }
                    if ui.button("Rename").clicked() {
                        self.editing_note_title = true;
                        self.refresh_rename_buffers();
                    }
                });
            }
        });
    }

    fn render_tab_row(&mut self, ui: &mut egui::Ui, note: &crate::models::Note) {
        panel_frame(egui::Color32::from_rgb(25, 27, 30))
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
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
                                let fill = if selected {
                                    ACTIVE_BG
                                } else {
                                    egui::Color32::TRANSPARENT
                                };

                                if selected
                                    && self.renaming_tab
                                    && self.rename_tab_id.as_deref() == Some(tab.id.as_str())
                                {
                                    ui.horizontal(|ui| {
                                        let response = ui.add_sized(
                                            [140.0, 28.0],
                                            egui::TextEdit::singleline(&mut self.rename_tab_title)
                                                .hint_text("Tab name")
                                                .margin(egui::Vec2::new(4.0, 2.0)),
                                        );

                                        let mut save = false;
                                        let mut cancel = false;

                                        if response.lost_focus() {
                                            save = true;
                                        }
                                        if ui.small_button("Save").clicked() {
                                            save = true;
                                        }
                                        if ui.small_button("Cancel").clicked() {
                                            cancel = true;
                                        }

                                        if cancel {
                                            self.renaming_tab = false;
                                            self.rename_tab_id = None;
                                            self.refresh_rename_buffers();
                                        } else if save {
                                            self.rename_current_tab();
                                            self.renaming_tab = false;
                                            self.rename_tab_id = None;
                                        }
                                    });
                                } else {
                                    let type_text = tab.tab_type.as_str();
                                    let mut job = egui::text::LayoutJob::default();
                                    job.append(
                                        &tab.title,
                                        0.0,
                                        egui::TextFormat::simple(
                                            egui::FontId::proportional(14.0),
                                            egui::Color32::WHITE,
                                        ),
                                    );
                                    job.append(
                                        format!(" ({type_text})").as_str(),
                                        0.0,
                                        egui::TextFormat::simple(
                                            egui::FontId::proportional(12.0),
                                            TEXT_MUTED,
                                        ),
                                    );
                                    let mut response = ui.add(
                                        egui::Button::new(job)
                                            .selected(selected)
                                            .fill(fill)
                                            // .frame(false)
                                            .sense(egui::Sense::click()),
                                    );
                                    response = response.on_hover_text(format!(
                                        "Type: {}\nRight-click for actions",
                                        tab.tab_type.label()
                                    ));

                                    response.context_menu(|ui| {
                                        ui.set_min_width(120.0);
                                        if ui.button("Rename").clicked() {
                                            if !selected {
                                                self.select_tab(tab.id.clone());
                                            }
                                            self.renaming_tab = true;
                                            self.rename_tab_id = Some(tab.id.clone());
                                            self.refresh_rename_buffers();
                                            ui.close();
                                        }
                                        ui.separator();
                                        if ui.button("Delete").clicked() {
                                            self.pending_confirm =
                                                Some(ConfirmAction::DeleteTab(tab.id.clone()));
                                            ui.close();
                                        }
                                    });

                                    if response.clicked() {
                                        self.select_tab(tab.id.clone());
                                    }
                                }
                            }
                        });
                    });
            });
    }

    fn render_tab_toolbar(&mut self, ui: &mut egui::Ui) {
        panel_frame(egui::Color32::from_rgb(25, 27, 30))
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // --- ADD NEW TAB SECTION ---
                    ui.label(egui::RichText::new("New:").small().color(TEXT_MUTED));

                    egui::ComboBox::from_id_salt("new-tab-type")
                        .width(100.0)
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
                        [160.0, 24.0],
                        egui::TextEdit::singleline(&mut self.new_tab_title)
                            .hint_text("Title")
                            .margin(egui::Vec2::new(4.0, 2.0)),
                    );

                    if ui
                        .add_sized([40.0, 24.0], egui::Button::new("Add").small())
                        .clicked()
                    {
                        self.add_tab();
                    }
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

        egui::Window::new("Confirm")
            .collapsible(false)
            .resizable(false)
            .show(context, |ui| {
                ui.label(message);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
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
                ui.add_space(12.0);
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
            Self::Idle => "Ready".to_owned(),
            Self::Unsaved => "Unsaved changes".to_owned(),
            Self::Saving => "Saving...".to_owned(),
            Self::Saved => "All changes saved".to_owned(),
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
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
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

#[cfg(test)]
fn note_header_action_area_size(width: f32) -> egui::Vec2 {
    egui::vec2(width, NOTE_HEADER_ACTION_HEIGHT)
}

#[cfg(test)]
fn kanban_column_header_action_area_size(width: f32) -> egui::Vec2 {
    egui::vec2(width, NOTE_HEADER_ACTION_HEIGHT)
}

#[cfg(test)]
fn kanban_column_area_size() -> egui::Vec2 {
    egui::vec2(KANBAN_COLUMN_WIDTH, 0.0)
}

#[cfg(test)]
fn kanban_card_text_area_size() -> egui::Vec2 {
    egui::vec2(KANBAN_CARD_TEXT_WIDTH, KANBAN_CARD_TEXT_HEIGHT)
}

#[cfg(test)]
fn tab_action_section_titles() -> [&'static str; 1] {
    ["Create Tab"]
}

fn panel_frame(fill: egui::Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .inner_margin(egui::Margin::same(12))
        .corner_radius(6)
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
            ui.add_space(20.0);
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
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(save_status.label())
                .small()
                .strong()
                .color(status_color(save_status)),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(workspace_path)
                    .small()
                    .color(TEXT_MUTED),
            );
            ui.separator();
            if let Some(tab) = selected_tab {
                ui.label(
                    egui::RichText::new(format!("{}: {}", tab.tab_type.as_str(), tab.file_name))
                        .small()
                        .color(TEXT_MUTED),
                );
            }
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
                        egui::vec2(KANBAN_COLUMN_WIDTH, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::Frame::new()
                                .fill(egui::Color32::from_rgb(25, 28, 32))
                                .inner_margin(egui::Margin::same(10))
                                .corner_radius(6)
                                .show(ui, |ui| {
                                    ui.set_width(KANBAN_COLUMN_WIDTH);
                                    ui.horizontal(|ui| {
                                        ui.heading(&board.columns[column_index].title);
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .add(egui::Button::new("+ Card").small())
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
                                            .corner_radius(4)
                                            .show(ui, |ui| {
                                                ui.set_width(KANBAN_CARD_TEXT_WIDTH);
                                                let card = &mut board.columns[column_index].cards
                                                    [card_index];
                                                if ui
                                                    .add_sized(
                                                        egui::vec2(
                                                            KANBAN_CARD_TEXT_WIDTH,
                                                            KANBAN_CARD_TEXT_HEIGHT,
                                                        ),
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
                    .corner_radius(4)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut item.done, "").changed() {
                                item.touch();
                                dirty = true;
                            }
                            if ui
                                .add_sized(
                                    [ui.available_width() - 50.0, 28.0],
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
                    .corner_radius(4)
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
                                    [ui.available_width() - 50.0, 28.0],
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
            // Swap logic or remove/insert logic
            let dest = destination as usize;
            column.cards.swap(card_index, dest);
        }
    }
}

fn shorten_path(path: &str) -> String {
    let components: Vec<&str> = path.split(std::path::MAIN_SEPARATOR).collect();
    if components.len() > 3 {
        format!(".../{}", components[components.len() - 1..].join("/"))
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::models::TabType;
    use crate::storage::Workspace;

    use super::{
        GimjiApp, KANBAN_CARD_TEXT_HEIGHT, KANBAN_CARD_TEXT_WIDTH, KANBAN_COLUMN_WIDTH,
        NOTE_HEADER_ACTION_HEIGHT, RecentWorkspaces, SaveStatus, kanban_card_text_area_size,
        kanban_column_area_size, kanban_column_header_action_area_size,
        note_header_action_area_size, note_matches_filter, tab_action_section_titles,
    };

    fn app_with_workspace(workspace: Workspace) -> GimjiApp {
        GimjiApp {
            workspace: Some(workspace),
            loaded: None,
            recent: RecentWorkspaces::default(),
            note_filter: String::new(),
            new_note_title: "New Note".to_owned(),
            new_tab_title: "Markdown".to_owned(),
            new_tab_type: TabType::Markdown,
            rename_note_title: String::new(),
            rename_tab_title: String::new(),
            renaming_tab: false,
            rename_tab_id: None,
            save_status: SaveStatus::Idle,
            pending_confirm: None,
            message: None,
            editing_note_title: false,
        }
    }

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
    fn tab_toolbar_only_contains_create_tab_actions() {
        let sections = tab_action_section_titles();

        assert_eq!(&sections[..], ["Create Tab"]);
    }

    #[test]
    fn tab_context_rename_targets_the_tab_opened_from_the_menu() {
        let temp_dir = tempfile::tempdir().expect("temporary workspace");
        let mut workspace = Workspace::create(temp_dir.path()).expect("workspace");
        let note_id = workspace.add_note("Project").expect("note");
        let first_tab_id = workspace
            .selected_tab_id()
            .expect("first tab selected")
            .to_owned();
        let second_tab_id = workspace
            .add_tab(&note_id, "Second", TabType::Markdown)
            .expect("second tab");

        let mut app = app_with_workspace(workspace);
        app.rename_tab_id = Some(first_tab_id.clone());
        app.rename_tab_title = "Renamed From Menu".to_owned();

        app.rename_current_tab();

        let workspace = app.workspace.as_ref().expect("workspace remains open");
        let renamed = workspace.find_tab(&first_tab_id).expect("renamed tab");
        let selected = workspace.find_tab(&second_tab_id).expect("selected tab");
        assert_eq!(renamed.title, "Renamed From Menu");
        assert_eq!(selected.title, "Second");
    }
}
