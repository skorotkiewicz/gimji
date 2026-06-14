use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::Result;
use crate::errors::AppError;
use crate::models::{
    AppConfig, CalendarData, KanbanBoard, Note, Tab, TabContent, TabType, TodoList,
};
use crate::storage::atomic::atomic_write;
use crate::storage::migration::{migrate_calendar, migrate_config, migrate_kanban, migrate_todo};

const CONFIG_FILE: &str = "config.json";
const CONTENT_DIR: &str = "content";
const BACKUPS_DIR: &str = "backups";
const APP_DIR: &str = ".app";

#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
    config: AppConfig,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeleteOptions {
    remove_local_files: bool,
}

impl DeleteOptions {
    pub const fn remove_local_files() -> Self {
        Self {
            remove_local_files: true,
        }
    }
}

impl Workspace {
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref().to_path_buf();
        ensure_workspace_dirs(&root)?;

        if root.join(CONFIG_FILE).exists() {
            return Self::open(root);
        }

        let workspace = Self {
            root,
            config: AppConfig::default(),
        };
        workspace.save_config()?;
        Ok(workspace)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref().to_path_buf();
        ensure_workspace_dirs(&root)?;

        let config_path = root.join(CONFIG_FILE);
        if !config_path.exists() {
            return Self::create(root);
        }

        let text = fs::read_to_string(&config_path)
            .map_err(|source| AppError::io(&config_path, source))?;
        let mut config: AppConfig =
            serde_json::from_str(&text).map_err(|source| AppError::json(&config_path, source))?;
        migrate_config(&mut config)?;

        for note in &config.notes {
            for tab in &note.tabs {
                validate_relative_content_path(&tab.file_name)?;
            }
        }

        Ok(Self { root, config })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn selected_note_id(&self) -> Option<&str> {
        self.config.selected_note_id.as_deref()
    }

    pub fn selected_tab_id(&self) -> Option<&str> {
        self.config.selected_tab_id.as_deref()
    }

    pub fn select_note(&mut self, note_id: &str) -> Result<()> {
        let note = self
            .config
            .notes
            .iter()
            .find(|note| note.id == note_id)
            .ok_or_else(|| AppError::NoteNotFound(note_id.to_owned()))?;
        self.config.selected_note_id = Some(note.id.clone());
        self.config.selected_tab_id = note.tabs.first().map(|tab| tab.id.clone());
        self.save_config()
    }

    pub fn select_tab(&mut self, tab_id: &str) -> Result<()> {
        self.find_tab(tab_id)?;
        self.config.selected_tab_id = Some(tab_id.to_owned());
        self.save_config()
    }

    pub fn add_note(&mut self, title: &str) -> Result<String> {
        let tab_id = crate::models::config::new_id();
        let file_name = make_content_file_name(title, "Markdown", TabType::Markdown, &tab_id);
        let first_tab = Tab {
            id: tab_id.clone(),
            title: "Markdown".to_owned(),
            tab_type: TabType::Markdown,
            file_name,
            created_at: crate::models::config::timestamp(),
            updated_at: crate::models::config::timestamp(),
        };
        let note = Note::new(title, first_tab.clone());
        let note_id = note.id.clone();

        self.write_default_content(&first_tab)?;
        self.config.selected_note_id = Some(note_id.clone());
        self.config.selected_tab_id = Some(tab_id);
        self.config.notes.push(note);
        self.save_config()?;

        Ok(note_id)
    }

    pub fn add_tab(&mut self, note_id: &str, title: &str, tab_type: TabType) -> Result<String> {
        let tab_id = crate::models::config::new_id();
        let note_title = self
            .config
            .notes
            .iter()
            .find(|note| note.id == note_id)
            .ok_or_else(|| AppError::NoteNotFound(note_id.to_owned()))?
            .title
            .clone();
        let file_name = make_content_file_name(&note_title, title, tab_type, &tab_id);
        let tab = Tab {
            id: tab_id.clone(),
            title: title.to_owned(),
            tab_type,
            file_name,
            created_at: crate::models::config::timestamp(),
            updated_at: crate::models::config::timestamp(),
        };

        self.write_default_content(&tab)?;
        let note = self.find_note_mut(note_id)?;
        note.tabs.push(tab);
        note.touch();
        self.config.selected_note_id = Some(note_id.to_owned());
        self.config.selected_tab_id = Some(tab_id.clone());
        self.save_config()?;

        Ok(tab_id)
    }

    pub fn rename_note(&mut self, note_id: &str, title: &str) -> Result<()> {
        let note = self.find_note_mut(note_id)?;
        note.title = title.to_owned();
        note.touch();
        self.save_config()
    }

    pub fn rename_tab(&mut self, tab_id: &str, title: &str) -> Result<()> {
        let tab = self.find_tab_mut(tab_id)?;
        tab.title = title.to_owned();
        tab.touch();
        self.save_config()
    }

    pub fn delete_note(&mut self, note_id: &str, options: DeleteOptions) -> Result<()> {
        let content_paths = if options.remove_local_files {
            let note = self
                .config
                .notes
                .iter()
                .find(|note| note.id == note_id)
                .ok_or_else(|| AppError::NoteNotFound(note_id.to_owned()))?;
            Some(
                note.tabs
                    .iter()
                    .map(|tab| self.content_path(tab))
                    .collect::<Result<Vec<_>>>()?,
            )
        } else {
            None
        };

        if let Some(paths) = &content_paths {
            self.remove_local_files(paths)?;
        }

        self.delete_note_config(note_id)?;

        Ok(())
    }

    pub fn delete_note_config(&mut self, note_id: &str) -> Result<()> {
        let before = self.config.notes.len();
        self.config.notes.retain(|note| note.id != note_id);
        if self.config.notes.len() == before {
            return Err(AppError::NoteNotFound(note_id.to_owned()));
        }

        if self.config.selected_note_id.as_deref() == Some(note_id) {
            let selected = self.config.notes.first();
            self.config.selected_note_id = selected.map(|note| note.id.clone());
            self.config.selected_tab_id =
                selected.and_then(|note| note.tabs.first().map(|tab| tab.id.clone()));
        }

        self.save_config()
    }

    pub fn delete_tab(&mut self, tab_id: &str, options: DeleteOptions) -> Result<()> {
        let content_paths = if options.remove_local_files {
            let tab = self.find_tab(tab_id)?;
            Some(vec![self.content_path(tab)?])
        } else {
            None
        };

        if let Some(paths) = &content_paths {
            self.remove_local_files(paths)?;
        }

        self.delete_tab_config(tab_id)?;

        Ok(())
    }

    pub fn delete_tab_config(&mut self, tab_id: &str) -> Result<()> {
        let note = self
            .config
            .notes
            .iter_mut()
            .find(|note| note.tabs.iter().any(|tab| tab.id == tab_id))
            .ok_or_else(|| AppError::TabNotFound(tab_id.to_owned()))?;

        if note.tabs.len() == 1 {
            return Err(AppError::InvalidPath(
                "a note must keep at least one tab".to_owned(),
            ));
        }

        note.tabs.retain(|tab| tab.id != tab_id);
        note.touch();
        if self.config.selected_tab_id.as_deref() == Some(tab_id) {
            self.config.selected_tab_id = note.tabs.first().map(|tab| tab.id.clone());
        }

        self.save_config()
    }

    pub fn save_tab_content(&self, tab_id: &str, content: &TabContent) -> Result<()> {
        let tab = self.find_tab(tab_id)?;
        if content.tab_type() != tab.tab_type {
            return Err(AppError::WrongContentType {
                expected: tab.tab_type.as_str(),
                actual: content.type_name(),
            });
        }

        match content {
            TabContent::Markdown(text) => self.write_text_content(tab, text),
            TabContent::Kanban(board) => self.write_json_content(tab, board),
            TabContent::Todo(list) => self.write_json_content(tab, list),
            TabContent::Calendar(calendar) => self.write_json_content(tab, calendar),
        }
    }

    pub fn load_tab_content(&self, tab_id: &str) -> Result<TabContent> {
        let tab = self.find_tab(tab_id)?;

        match tab.tab_type {
            TabType::Markdown => Ok(TabContent::Markdown(self.read_text_content(tab)?)),
            TabType::Kanban => {
                let board: KanbanBoard = self.read_json_content(tab)?;
                Ok(TabContent::Kanban(migrate_kanban(board)?))
            }
            TabType::Todo => {
                let list: TodoList = self.read_json_content(tab)?;
                Ok(TabContent::Todo(migrate_todo(list)?))
            }
            TabType::Calendar => {
                let calendar: CalendarData = self.read_json_content(tab)?;
                Ok(TabContent::Calendar(migrate_calendar(calendar)?))
            }
        }
    }

    pub fn save_config(&self) -> Result<()> {
        let path = self.root.join(CONFIG_FILE);
        let bytes = serde_json::to_vec_pretty(&self.config)
            .map_err(|source| AppError::json(&path, source))?;
        atomic_write(&path, &bytes)
    }

    fn write_default_content(&self, tab: &Tab) -> Result<()> {
        match tab.tab_type {
            TabType::Markdown => self.write_text_content(tab, ""),
            TabType::Kanban => self.write_json_content(tab, &KanbanBoard::default()),
            TabType::Todo => self.write_json_content(tab, &TodoList::default()),
            TabType::Calendar => self.write_json_content(tab, &CalendarData::default()),
        }
    }

    fn read_text_content(&self, tab: &Tab) -> Result<String> {
        let path = self.content_path(tab)?;
        fs::read_to_string(&path).map_err(|source| AppError::io(&path, source))
    }

    fn write_text_content(&self, tab: &Tab, text: &str) -> Result<()> {
        let path = self.content_path(tab)?;
        atomic_write(&path, text.as_bytes())
    }

    fn read_json_content<T: serde::de::DeserializeOwned>(&self, tab: &Tab) -> Result<T> {
        let path = self.content_path(tab)?;
        let text = fs::read_to_string(&path).map_err(|source| AppError::io(&path, source))?;
        serde_json::from_str(&text).map_err(|source| AppError::json(&path, source))
    }

    fn write_json_content<T: serde::Serialize>(&self, tab: &Tab, value: &T) -> Result<()> {
        let path = self.content_path(tab)?;
        let bytes =
            serde_json::to_vec_pretty(value).map_err(|source| AppError::json(&path, source))?;
        atomic_write(&path, &bytes)
    }

    fn content_path(&self, tab: &Tab) -> Result<PathBuf> {
        validate_relative_content_path(&tab.file_name)?;
        Ok(self.root.join(&tab.file_name))
    }

    fn remove_local_files(&self, paths: &[PathBuf]) -> Result<()> {
        for path in paths {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => return Err(AppError::io(path, source)),
            }
        }

        Ok(())
    }

    fn find_note_mut(&mut self, note_id: &str) -> Result<&mut Note> {
        self.config
            .notes
            .iter_mut()
            .find(|note| note.id == note_id)
            .ok_or_else(|| AppError::NoteNotFound(note_id.to_owned()))
    }

    pub fn find_tab(&self, tab_id: &str) -> Result<&Tab> {
        self.config
            .notes
            .iter()
            .flat_map(|note| note.tabs.iter())
            .find(|tab| tab.id == tab_id)
            .ok_or_else(|| AppError::TabNotFound(tab_id.to_owned()))
    }

    fn find_tab_mut(&mut self, tab_id: &str) -> Result<&mut Tab> {
        self.config
            .notes
            .iter_mut()
            .flat_map(|note| note.tabs.iter_mut())
            .find(|tab| tab.id == tab_id)
            .ok_or_else(|| AppError::TabNotFound(tab_id.to_owned()))
    }
}

pub fn sanitize_file_stem(text: &str) -> String {
    let mut stem = String::new();
    let mut last_was_dash = false;

    for ch in text.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            stem.push(ch);
            last_was_dash = false;
        } else if !last_was_dash && !stem.is_empty() {
            stem.push('-');
            last_was_dash = true;
        }
    }

    while stem.ends_with('-') {
        stem.pop();
    }

    if stem.is_empty() {
        "file".to_owned()
    } else {
        stem
    }
}

pub fn make_content_file_name(
    note_title: &str,
    tab_title: &str,
    tab_type: TabType,
    tab_id: &str,
) -> String {
    let id_part: String = tab_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(8)
        .collect();
    format!(
        "{CONTENT_DIR}/{}-{}-{}{}",
        sanitize_file_stem(note_title),
        sanitize_file_stem(tab_title),
        if id_part.is_empty() { "tab" } else { &id_part },
        tab_type.extension()
    )
}

pub fn validate_relative_content_path(path: &str) -> Result<()> {
    let path = Path::new(path);

    if path.is_absolute() {
        return Err(AppError::InvalidPath(path.display().to_string()));
    }

    let mut components = path.components();
    match components.next() {
        Some(Component::Normal(first)) if first == CONTENT_DIR => {}
        _ => return Err(AppError::InvalidPath(path.display().to_string())),
    }

    for component in components {
        match component {
            Component::Normal(_) => {}
            _ => return Err(AppError::InvalidPath(path.display().to_string())),
        }
    }

    Ok(())
}

fn ensure_workspace_dirs(root: &Path) -> Result<()> {
    fs::create_dir_all(root).map_err(|source| AppError::io(root, source))?;
    for dir in [CONTENT_DIR, BACKUPS_DIR, APP_DIR] {
        let path = root.join(dir);
        fs::create_dir_all(&path).map_err(|source| AppError::io(&path, source))?;
    }
    Ok(())
}
