use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub version: u32,
    pub selected_note_id: Option<String>,
    pub selected_tab_id: Option<String>,
    pub notes: Vec<Note>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            selected_note_id: None,
            selected_tab_id: None,
            notes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub tabs: Vec<Tab>,
}

impl Note {
    pub fn new(title: impl Into<String>, first_tab: Tab) -> Self {
        let now = timestamp();

        Self {
            id: new_id(),
            title: title.into(),
            created_at: now.clone(),
            updated_at: now,
            tabs: vec![first_tab],
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = timestamp();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tab {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub tab_type: TabType,
    pub file_name: String,
    pub created_at: String,
    pub updated_at: String,
}

impl Tab {
    pub fn new(title: impl Into<String>, tab_type: TabType, file_name: impl Into<String>) -> Self {
        let now = timestamp();

        Self {
            id: new_id(),
            title: title.into(),
            tab_type,
            file_name: file_name.into(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = timestamp();
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TabType {
    Markdown,
    Kanban,
    Todo,
    Calendar,
}

impl TabType {
    pub const ALL: [Self; 4] = [Self::Markdown, Self::Kanban, Self::Todo, Self::Calendar];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Kanban => "kanban",
            Self::Todo => "todo",
            Self::Calendar => "calendar",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Markdown => ".md",
            Self::Kanban => ".kanban.json",
            Self::Todo => ".todo.json",
            Self::Calendar => ".calendar.json",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Markdown => "Markdown",
            Self::Kanban => "Kanban",
            Self::Todo => "Todo",
            Self::Calendar => "Calendar",
        }
    }
}

impl std::fmt::Display for TabType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn timestamp() -> String {
    Utc::now().to_rfc3339()
}
