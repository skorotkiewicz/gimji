use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AppConfig {
    pub selected_note_id: Option<String>,
    pub selected_tab_id: Option<String>,
    pub notes: Vec<Note>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown_files: Option<Vec<MarkdownFile>>,
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
            markdown_files: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn markdown_entries(&self) -> Vec<MarkdownFile> {
        self.markdown_files.clone().unwrap_or_else(|| {
            vec![MarkdownFile {
                id: self.id.clone(),
                title: self.title.clone(),
                file_name: self.file_name.clone(),
                collapsed: false,
            }]
        })
    }

    pub fn content_files(&self) -> impl Iterator<Item = &str> {
        let files = self
            .markdown_files
            .as_ref()
            .filter(|_| self.tab_type == TabType::Markdown);
        files
            .into_iter()
            .flatten()
            .map(|file| file.file_name.as_str())
            .chain(files.is_none().then_some(self.file_name.as_str()))
    }

    pub fn touch(&mut self) {
        self.updated_at = timestamp();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarkdownFile {
    pub id: String,
    pub title: String,
    pub file_name: String,
    #[serde(default)]
    pub collapsed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownDocument {
    pub file: MarkdownFile,
    pub text: String,
}

impl MarkdownDocument {
    pub fn new(title: &str) -> Self {
        let id = new_id();
        Self {
            file: MarkdownFile {
                file_name: format!("content/{id}.md"),
                id,
                title: title.to_owned(),
                collapsed: false,
            },
            text: String::new(),
        }
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
