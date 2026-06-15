#[cfg(feature = "s3")]
use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui;

use crate::models::{TabContent, TabType};
#[cfg(feature = "s3")]
use crate::storage::S3ConnectionSettings;
use crate::storage::{DeleteOptions, Workspace};

mod dialogs;
mod editors;
mod recent;
mod selection;
mod sidebar;
mod tabs;

use recent::{RecentWorkspaces, RecentWorkspacesStore, recent_workspaces_path};

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
#[cfg(feature = "s3")]
const DEFAULT_S3_REGION: &str = "us-east-1";
#[cfg(feature = "s3")]
const ENV_S3_ENDPOINT: &str = "GIMJI_S3_ENDPOINT";
#[cfg(feature = "s3")]
const ENV_S3_REGION: &str = "GIMJI_S3_REGION";
#[cfg(feature = "s3")]
const ENV_S3_BUCKET: &str = "GIMJI_S3_BUCKET";
#[cfg(feature = "s3")]
const ENV_S3_PREFIX: &str = "GIMJI_S3_PREFIX";
#[cfg(feature = "s3")]
const ENV_S3_ACCESS_KEY: &str = "GIMJI_S3_ACCESS_KEY";
#[cfg(feature = "s3")]
const ENV_S3_SECRET_KEY: &str = "GIMJI_S3_SECRET_KEY";

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
    rename_note_title: String,
    rename_tab_title: String,
    renaming_tab: bool,
    rename_tab_id: Option<String>,
    save_status: SaveStatus,
    pending_confirm: Option<ConfirmAction>,
    remove_local_files_on_delete: bool,
    #[cfg(feature = "s3")]
    s3_endpoint_url: String,
    #[cfg(feature = "s3")]
    s3_region: String,
    #[cfg(feature = "s3")]
    s3_bucket: String,
    #[cfg(feature = "s3")]
    s3_prefix: String,
    #[cfg(feature = "s3")]
    s3_access_key_id: String,
    #[cfg(feature = "s3")]
    s3_secret_access_key: String,
    #[cfg(feature = "s3")]
    s3_connection_status: S3ConnectionStatus,
    #[cfg(feature = "s3")]
    s3_settings_expanded: bool,
    message: Option<String>,
    // UI State
    editing_note_title: bool,
}

impl GimjiApp {
    fn new(creation_context: &eframe::CreationContext<'_>) -> Self {
        configure_theme(&creation_context.egui_ctx);
        #[cfg(feature = "s3")]
        let initial_s3_settings = initial_s3_connection_settings_from_environment();

        Self {
            workspace: None,
            loaded: None,
            recent: load_recent_workspaces(),
            note_filter: String::new(),
            new_note_title: "New Note".to_owned(),
            rename_note_title: String::new(),
            rename_tab_title: String::new(),
            renaming_tab: false,
            rename_tab_id: None,
            save_status: SaveStatus::Idle,
            pending_confirm: None,
            remove_local_files_on_delete: false,
            #[cfg(feature = "s3")]
            s3_endpoint_url: initial_s3_settings.endpoint_url,
            #[cfg(feature = "s3")]
            s3_region: initial_s3_settings.region,
            #[cfg(feature = "s3")]
            s3_bucket: initial_s3_settings.bucket,
            #[cfg(feature = "s3")]
            s3_prefix: initial_s3_settings.prefix,
            #[cfg(feature = "s3")]
            s3_access_key_id: initial_s3_settings.access_key_id,
            #[cfg(feature = "s3")]
            s3_secret_access_key: initial_s3_settings.secret_access_key,
            #[cfg(feature = "s3")]
            s3_connection_status: S3ConnectionStatus::Idle,
            #[cfg(feature = "s3")]
            s3_settings_expanded: false,
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
                self.save_recent_workspaces();
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
                self.save_recent_workspaces();
                self.load_selected_content();
            }
            Err(error) => self.set_error(error.to_string()),
        }
    }

    fn remove_recent_workspace(&mut self, path: &Path) {
        if self.recent.remove(path) {
            self.save_recent_workspaces();
        }
    }

    fn save_recent_workspaces(&self) {
        if let Some(path) = recent_workspaces_path() {
            RecentWorkspacesStore::save(&path, &self.recent);
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

    fn add_tab(&mut self, tab_type: TabType) {
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
            match workspace.add_tab(&note_id, tab_type.label(), tab_type) {
                Ok(tab_id) => {
                    self.loaded = None;
                    self.load_selected_content();
                    self.renaming_tab = true;
                    self.rename_tab_id = Some(tab_id);
                    self.refresh_rename_buffers();
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

    fn cancel_note_title_edit(&mut self) {
        self.editing_note_title = false;
        self.refresh_rename_buffers();
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

    fn save_tab_title_edit(&mut self) {
        if self.rename_tab_title.trim().is_empty() {
            return;
        }

        self.rename_current_tab();
        self.renaming_tab = false;
        self.rename_tab_id = None;
    }

    fn cancel_tab_title_edit(&mut self) {
        self.renaming_tab = false;
        self.rename_tab_id = None;
        self.refresh_rename_buffers();
    }

    fn refresh_rename_buffers(&mut self) {
        if let Some(note) = self.current_note() {
            self.rename_note_title = note.title.clone();
        }
        if let Some(tab) = self.current_tab() {
            self.rename_tab_title = tab.title.clone();
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
            self.add_tab(TabType::Markdown);
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

    #[cfg(feature = "s3")]
    fn s3_connection_settings(&self) -> S3ConnectionSettings {
        S3ConnectionSettings {
            endpoint_url: self.s3_endpoint_url.trim().to_owned(),
            region: self.s3_region.trim().to_owned(),
            bucket: self.s3_bucket.trim().to_owned(),
            prefix: self.s3_prefix.trim().to_owned(),
            access_key_id: self.s3_access_key_id.trim().to_owned(),
            secret_access_key: self.s3_secret_access_key.trim().to_owned(),
        }
    }

    #[cfg(feature = "s3")]
    fn toggle_s3_settings(&mut self) {
        self.s3_settings_expanded = !self.s3_settings_expanded;
    }

    #[cfg(feature = "s3")]
    fn test_s3_connection(&mut self) {
        let settings = self.s3_connection_settings();
        self.s3_connection_status = S3ConnectionStatus::Testing;

        let result = tokio::runtime::Runtime::new()
            .map_err(|error| error.to_string())
            .and_then(|runtime| {
                runtime
                    .block_on(settings.test_connection())
                    .map_err(|error| error.to_string())
            });

        match result {
            Ok(()) => {
                self.s3_connection_status = S3ConnectionStatus::Connected;
                self.message = Some("S3 connection successful.".to_owned());
            }
            Err(error) => {
                self.s3_connection_status = S3ConnectionStatus::Error(error.clone());
                self.message = Some(error);
            }
        }
    }

    #[cfg(feature = "s3")]
    fn backup_workspace_to_s3(&mut self) {
        self.flush_current();

        let Some(workspace) = self.workspace.as_ref() else {
            self.message = Some("Open a workspace before backing up to S3.".to_owned());
            return;
        };

        let settings = self.s3_connection_settings();
        self.s3_connection_status = S3ConnectionStatus::Testing;

        let result = tokio::runtime::Runtime::new()
            .map_err(|error| error.to_string())
            .and_then(|runtime| {
                runtime
                    .block_on(settings.backup_workspace(workspace))
                    .map_err(|error| error.to_string())
            });

        match result {
            Ok(()) => {
                self.s3_connection_status = S3ConnectionStatus::Connected;
                self.message = Some("S3 backup successful.".to_owned());
            }
            Err(error) => {
                self.s3_connection_status = S3ConnectionStatus::Error(error.clone());
                self.message = Some(error);
            }
        }
    }

    #[cfg(feature = "s3")]
    fn restore_workspace_from_s3(&mut self) {
        self.flush_current();

        let Some(root) = self
            .workspace
            .as_ref()
            .map(|workspace| workspace.root().to_path_buf())
        else {
            self.message = Some("Open a workspace before restoring from S3.".to_owned());
            return;
        };

        let settings = self.s3_connection_settings();
        self.s3_connection_status = S3ConnectionStatus::Testing;

        let result = tokio::runtime::Runtime::new()
            .map_err(|error| error.to_string())
            .and_then(|runtime| {
                let workspace = self
                    .workspace
                    .as_ref()
                    .ok_or_else(|| "Open a workspace before restoring from S3.".to_owned())?;
                runtime
                    .block_on(settings.restore_workspace(workspace))
                    .map_err(|error| error.to_string())
            });

        match result.and_then(|()| Workspace::open(&root).map_err(|error| error.to_string())) {
            Ok(workspace) => {
                self.workspace = Some(workspace);
                self.loaded = None;
                self.load_selected_content();
                self.s3_connection_status = S3ConnectionStatus::Connected;
                self.message = Some("S3 restore successful.".to_owned());
            }
            Err(error) => {
                self.s3_connection_status = S3ConnectionStatus::Error(error.clone());
                self.message = Some(error);
            }
        }
    }

    #[cfg(feature = "s3")]
    fn request_s3_restore(&mut self) {
        self.pending_confirm = Some(ConfirmAction::RestoreWorkspaceFromS3);
        self.remove_local_files_on_delete = false;
    }

    fn request_delete(&mut self, action: ConfirmAction) {
        self.pending_confirm = Some(action);
        self.remove_local_files_on_delete = false;
    }

    fn confirm_action(&mut self) {
        let Some(action) = self.pending_confirm.take() else {
            return;
        };
        let options = if self.remove_local_files_on_delete {
            DeleteOptions::remove_local_files()
        } else {
            DeleteOptions::default()
        };

        self.flush_current();
        match action {
            ConfirmAction::DeleteNote(note_id) => {
                if let Some(workspace) = &mut self.workspace {
                    match workspace.delete_note(&note_id, options) {
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
                    match workspace.delete_tab(&tab_id, options) {
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
            #[cfg(feature = "s3")]
            ConfirmAction::RestoreWorkspaceFromS3 => {
                self.restore_workspace_from_s3();
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
        self.save_recent_workspaces();
    }
}

impl GimjiApp {
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

                // 2. Tab Row (with inline "+" to add tabs)
                self.render_tab_row(ui, &note);

                ui.add_space(8.0);

                // 3. Status Strip
                render_status_strip(
                    ui,
                    &workspace_path,
                    selected_tab.as_ref(),
                    &self.save_status,
                );

                ui.add_space(8.0);

                // 4. Content Area
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

    fn render_content(&mut self, ui: &mut egui::Ui) {
        let Some(loaded) = &mut self.loaded else {
            render_empty_state(ui, "No content", "Select a tab to load its content.");
            return;
        };

        let dirty = match &mut loaded.content {
            TabContent::Markdown(markdown) => editors::render_markdown(ui, markdown),
            TabContent::Kanban(board) => editors::render_kanban(ui, board),
            TabContent::Todo(todo) => editors::render_todo(ui, todo),
            TabContent::Calendar(calendar) => editors::render_calendar(ui, calendar),
        };

        if dirty {
            self.mark_dirty();
        }
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

#[cfg(feature = "s3")]
#[derive(Debug, Clone)]
enum S3ConnectionStatus {
    Idle,
    Testing,
    Connected,
    Error(String),
}

#[cfg(feature = "s3")]
impl S3ConnectionStatus {
    fn label(&self) -> String {
        match self {
            Self::Idle => "Not connected".to_owned(),
            Self::Testing => "Testing...".to_owned(),
            Self::Connected => "Connected".to_owned(),
            Self::Error(message) => format!("Error: {message}"),
        }
    }
}

#[derive(Debug, Clone)]
enum ConfirmAction {
    DeleteNote(String),
    DeleteTab(String),
    #[cfg(feature = "s3")]
    RestoreWorkspaceFromS3,
}

fn load_recent_workspaces() -> RecentWorkspaces {
    recent_workspaces_path()
        .as_deref()
        .map(RecentWorkspacesStore::load)
        .unwrap_or_default()
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

#[cfg(feature = "s3")]
fn initial_s3_connection_settings_from_environment() -> S3ConnectionSettings {
    initial_s3_connection_settings(|key| env::var(key).ok())
}

#[cfg(feature = "s3")]
fn initial_s3_connection_settings(
    mut get_env: impl FnMut(&str) -> Option<String>,
) -> S3ConnectionSettings {
    S3ConnectionSettings {
        endpoint_url: get_env(ENV_S3_ENDPOINT).unwrap_or_default(),
        region: get_env(ENV_S3_REGION).unwrap_or_else(|| DEFAULT_S3_REGION.to_owned()),
        bucket: get_env(ENV_S3_BUCKET).unwrap_or_default(),
        prefix: get_env(ENV_S3_PREFIX).unwrap_or_default(),
        access_key_id: get_env(ENV_S3_ACCESS_KEY).unwrap_or_default(),
        secret_access_key: get_env(ENV_S3_SECRET_KEY).unwrap_or_default(),
    }
}

#[cfg(test)]
fn note_header_action_area_size(width: f32) -> egui::Vec2 {
    egui::vec2(width, NOTE_HEADER_ACTION_HEIGHT)
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::models::{Tab, TabContent, TabType};
    use crate::storage::Workspace;

    use super::editors::{
        KANBAN_CARD_TEXT_HEIGHT, KANBAN_CARD_TEXT_WIDTH, KANBAN_COLUMN_WIDTH,
        kanban_card_text_area_size, kanban_column_area_size, kanban_column_header_action_area_size,
        new_calendar_event, new_todo_item,
    };
    use super::selection::{SelectedContent, selected_content_for_workspace};
    #[cfg(feature = "s3")]
    use super::{ConfirmAction, S3ConnectionStatus, initial_s3_connection_settings};
    use super::{
        GimjiApp, NOTE_HEADER_ACTION_HEIGHT, RecentWorkspaces, RecentWorkspacesStore, SaveStatus,
        note_header_action_area_size,
    };

    #[test]
    fn editor_layout_helpers_live_with_editor_renderers() {
        let column_size = kanban_column_area_size();
        let card_size = kanban_card_text_area_size();

        assert_eq!(column_size.x, KANBAN_COLUMN_WIDTH);
        assert_eq!(column_size.y, 0.0);
        assert_eq!(card_size.x, KANBAN_CARD_TEXT_WIDTH);
        assert_eq!(card_size.y, KANBAN_CARD_TEXT_HEIGHT);
    }

    #[test]
    fn selection_module_loads_the_selected_tab_content() {
        let temp_dir = tempfile::tempdir().expect("temporary workspace");
        let mut workspace = Workspace::create(temp_dir.path()).expect("workspace");
        workspace.add_note("Project").expect("note");
        let tab_id = workspace
            .selected_tab_id()
            .expect("selected tab")
            .to_owned();

        workspace
            .save_tab_content(
                &tab_id,
                &TabContent::Markdown("# Selected project".to_owned()),
            )
            .expect("save selected content");

        let selection =
            selected_content_for_workspace(Some(&workspace), None).expect("selected content");

        assert_eq!(
            selection,
            SelectedContent::Loaded {
                tab_id,
                content: TabContent::Markdown("# Selected project".to_owned())
            }
        );
    }

    #[test]
    fn dialog_module_defines_confirm_button_labels() {
        assert_eq!(super::dialogs::confirm_button_label_for_delete(), "Delete");
    }

    fn app_with_workspace(workspace: Workspace) -> GimjiApp {
        GimjiApp {
            workspace: Some(workspace),
            loaded: None,
            recent: RecentWorkspaces::default(),
            note_filter: String::new(),
            new_note_title: "New Note".to_owned(),
            rename_note_title: String::new(),
            rename_tab_title: String::new(),
            renaming_tab: false,
            rename_tab_id: None,
            save_status: SaveStatus::Idle,
            pending_confirm: None,
            remove_local_files_on_delete: false,
            #[cfg(feature = "s3")]
            s3_endpoint_url: String::new(),
            #[cfg(feature = "s3")]
            s3_region: "us-east-1".to_owned(),
            #[cfg(feature = "s3")]
            s3_bucket: String::new(),
            #[cfg(feature = "s3")]
            s3_prefix: String::new(),
            #[cfg(feature = "s3")]
            s3_access_key_id: String::new(),
            #[cfg(feature = "s3")]
            s3_secret_access_key: String::new(),
            #[cfg(feature = "s3")]
            s3_connection_status: S3ConnectionStatus::Idle,
            #[cfg(feature = "s3")]
            s3_settings_expanded: false,
            message: None,
            editing_note_title: false,
        }
    }

    #[test]
    fn note_filter_ignores_case_and_surrounding_whitespace() {
        assert!(super::sidebar::note_matches_filter(
            "Project Notes",
            " project "
        ));
        assert!(super::sidebar::note_matches_filter(
            "Project Notes",
            "NOTES"
        ));
        assert!(super::sidebar::note_matches_filter("Project Notes", ""));
        assert!(!super::sidebar::note_matches_filter(
            "Project Notes",
            "archive"
        ));
    }

    #[test]
    fn add_tab_creates_default_named_tab_and_enters_inline_rename() {
        let temp_dir = tempfile::tempdir().expect("temp workspace");
        let mut workspace = Workspace::create(temp_dir.path()).expect("workspace");
        workspace.add_note("Project").expect("note");
        let mut app = app_with_workspace(workspace);

        app.add_tab(TabType::Kanban);

        let workspace = app.workspace.as_ref().expect("workspace");
        let tab_id = workspace
            .selected_tab_id()
            .expect("selected tab")
            .to_owned();
        let tab = workspace.find_tab(&tab_id).expect("tab");
        assert_eq!(tab.title, TabType::Kanban.label());
        assert!(app.renaming_tab);
        assert_eq!(app.rename_tab_id.as_deref(), Some(tab_id.as_str()));
        assert_eq!(app.rename_tab_title, TabType::Kanban.label());
    }

    #[test]
    fn removing_recent_workspace_deletes_only_selected_path() {
        let first = PathBuf::from("/tmp/gimji-first");
        let second = PathBuf::from("/tmp/gimji-second");
        let mut recent = RecentWorkspaces {
            paths: vec![first.clone(), second.clone()],
        };

        recent.remove(&first);

        assert_eq!(recent.paths, vec![second]);
    }

    #[test]
    fn recent_workspace_store_round_trips_through_explicit_path() {
        let temp_dir = tempfile::tempdir().expect("temporary config dir");
        let store_path = temp_dir.path().join("config/recent_workspaces.json");
        let recent = RecentWorkspaces {
            paths: vec![
                PathBuf::from("/tmp/gimji-first"),
                PathBuf::from("/tmp/gimji-second"),
            ],
        };

        RecentWorkspacesStore::save(&store_path, &recent);
        let loaded = RecentWorkspacesStore::load(&store_path);

        assert_eq!(loaded.paths, recent.paths);
    }

    #[test]
    fn editor_add_actions_create_blank_todo_and_event_fields() {
        let todo = new_todo_item();
        let event = new_calendar_event("2026-06-15".to_owned());

        assert_eq!(todo.text, "");
        assert!(!todo.done);
        assert_eq!(event.date, "2026-06-15");
        assert_eq!(event.title, "");
        assert_eq!(event.description, "");
    }

    #[test]
    fn deleting_recent_workspace_from_menu_does_not_open_it() {
        let open_dir = tempfile::tempdir().expect("open workspace");
        let recent_dir = tempfile::tempdir().expect("recent workspace");
        let open_workspace = Workspace::create(open_dir.path()).expect("workspace");
        let open_root = open_workspace.root().to_path_buf();
        let recent_path = recent_dir.path().to_path_buf();
        let mut app = app_with_workspace(open_workspace);
        app.recent.paths = vec![open_root.clone(), recent_path.clone()];

        app.remove_recent_workspace(&recent_path);

        assert_eq!(
            app.workspace.as_ref().expect("workspace").root(),
            open_root.as_path()
        );
        assert_eq!(app.recent.paths, vec![open_root]);
    }

    #[cfg(feature = "s3")]
    #[test]
    fn app_builds_s3_connection_settings_from_form_fields() {
        let temp_dir = tempfile::tempdir().expect("temp workspace");
        let workspace = Workspace::create(temp_dir.path()).expect("workspace");
        let mut app = app_with_workspace(workspace);

        app.s3_endpoint_url = " http://192.168.0.125:9000 ".to_owned();
        app.s3_region = " us-east-1 ".to_owned();
        app.s3_bucket = " gimji ".to_owned();
        app.s3_prefix = " projects/gimji-main ".to_owned();
        app.s3_access_key_id = " minioadmin ".to_owned();
        app.s3_secret_access_key = " minioadmin ".to_owned();

        let settings = app.s3_connection_settings();

        assert_eq!(settings.endpoint_url, "http://192.168.0.125:9000");
        assert_eq!(settings.region, "us-east-1");
        assert_eq!(settings.bucket, "gimji");
        assert_eq!(settings.prefix, "projects/gimji-main");
        assert_eq!(settings.access_key_id, "minioadmin");
        assert_eq!(settings.secret_access_key, "minioadmin");
    }

    #[cfg(feature = "s3")]
    #[test]
    fn s3_form_defaults_can_be_loaded_from_environment_variables() {
        let settings = initial_s3_connection_settings(|key| match key {
            "GIMJI_S3_ENDPOINT" => Some("http://192.168.0.125:9000".to_owned()),
            "GIMJI_S3_REGION" => Some("us-east-1".to_owned()),
            "GIMJI_S3_BUCKET" => Some("storage".to_owned()),
            "GIMJI_S3_PREFIX" => Some("projects/gimji-main".to_owned()),
            "GIMJI_S3_ACCESS_KEY" => Some("minioadmin".to_owned()),
            "GIMJI_S3_SECRET_KEY" => Some("minioadmin".to_owned()),
            _ => None,
        });

        assert_eq!(settings.endpoint_url, "http://192.168.0.125:9000");
        assert_eq!(settings.region, "us-east-1");
        assert_eq!(settings.bucket, "storage");
        assert_eq!(settings.prefix, "projects/gimji-main");
        assert_eq!(settings.access_key_id, "minioadmin");
        assert_eq!(settings.secret_access_key, "minioadmin");
    }

    #[cfg(feature = "s3")]
    #[test]
    fn s3_settings_are_hidden_until_section_is_toggled() {
        let temp_dir = tempfile::tempdir().expect("temp workspace");
        let workspace = Workspace::create(temp_dir.path()).expect("workspace");
        let mut app = app_with_workspace(workspace);

        assert!(!app.s3_settings_expanded);

        app.toggle_s3_settings();

        assert!(app.s3_settings_expanded);
    }

    #[cfg(feature = "s3")]
    #[test]
    fn s3_connection_validation_failure_does_not_change_save_status() {
        let temp_dir = tempfile::tempdir().expect("temp workspace");
        let workspace = Workspace::create(temp_dir.path()).expect("workspace");
        let mut app = app_with_workspace(workspace);

        app.test_s3_connection();

        assert!(matches!(app.save_status, SaveStatus::Idle));
        assert!(matches!(
            app.s3_connection_status,
            S3ConnectionStatus::Error(_)
        ));
    }

    #[cfg(feature = "s3")]
    #[test]
    fn s3_backup_validation_failure_does_not_change_save_status() {
        let temp_dir = tempfile::tempdir().expect("temp workspace");
        let workspace = Workspace::create(temp_dir.path()).expect("workspace");
        let mut app = app_with_workspace(workspace);

        app.backup_workspace_to_s3();

        assert!(matches!(app.save_status, SaveStatus::Idle));
        assert!(matches!(
            app.s3_connection_status,
            S3ConnectionStatus::Error(_)
        ));
    }

    #[cfg(feature = "s3")]
    #[test]
    fn s3_restore_button_requests_confirmation_before_overwriting_workspace() {
        let temp_dir = tempfile::tempdir().expect("temp workspace");
        let workspace = Workspace::create(temp_dir.path()).expect("workspace");
        let mut app = app_with_workspace(workspace);

        app.request_s3_restore();

        assert!(matches!(
            app.pending_confirm,
            Some(ConfirmAction::RestoreWorkspaceFromS3)
        ));
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
    fn tab_button_job_shows_title_only_and_type_color() {
        let tab = Tab::new("Docs", TabType::Todo, "Docs.todo.json");
        let job = super::tabs::tab_button_job(&tab);

        assert_eq!(job.text, "Docs");
        assert_eq!(job.sections.len(), 1);
        assert_eq!(
            job.sections[0].format.color,
            super::tabs::tab_type_color(TabType::Todo)
        );
    }

    #[test]
    fn each_tab_type_has_distinct_tab_color() {
        let colors: Vec<_> = TabType::ALL
            .iter()
            .map(|tab_type| super::tabs::tab_type_color(*tab_type))
            .collect();

        for (index, left) in colors.iter().enumerate() {
            for right in colors.iter().skip(index + 1) {
                assert_ne!(left, right);
            }
        }
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

    #[test]
    fn rename_edit_actions_save_or_cancel_note_and_tab_edits() {
        let temp_dir = tempfile::tempdir().expect("temporary workspace");
        let mut workspace = Workspace::create(temp_dir.path()).expect("workspace");
        let note_id = workspace.add_note("Project").expect("note");
        let tab_id = workspace
            .selected_tab_id()
            .expect("selected tab")
            .to_owned();
        let mut app = app_with_workspace(workspace);

        app.editing_note_title = true;
        app.rename_note_title = "Draft Note".to_owned();
        app.cancel_note_title_edit();
        assert!(!app.editing_note_title);
        assert_eq!(
            app.workspace
                .as_ref()
                .expect("workspace")
                .config()
                .notes
                .iter()
                .find(|note| note.id == note_id)
                .expect("note")
                .title,
            "Project"
        );

        app.editing_note_title = true;
        app.rename_note_title = "Renamed Note".to_owned();
        app.rename_current_note();
        assert!(!app.editing_note_title);
        assert_eq!(
            app.workspace
                .as_ref()
                .expect("workspace")
                .config()
                .notes
                .iter()
                .find(|note| note.id == note_id)
                .expect("note")
                .title,
            "Renamed Note"
        );

        app.renaming_tab = true;
        app.rename_tab_id = Some(tab_id.clone());
        app.rename_tab_title = "Draft Tab".to_owned();
        app.cancel_tab_title_edit();
        assert!(!app.renaming_tab);
        assert_eq!(app.rename_tab_id, None);
        assert_eq!(
            app.workspace
                .as_ref()
                .expect("workspace")
                .find_tab(&tab_id)
                .expect("tab")
                .title,
            "Markdown"
        );

        app.renaming_tab = true;
        app.rename_tab_id = Some(tab_id.clone());
        app.rename_tab_title = "Renamed Tab".to_owned();
        app.save_tab_title_edit();
        assert!(!app.renaming_tab);
        assert_eq!(app.rename_tab_id, None);
        assert_eq!(
            app.workspace
                .as_ref()
                .expect("workspace")
                .find_tab(&tab_id)
                .expect("tab")
                .title,
            "Renamed Tab"
        );
    }
}
