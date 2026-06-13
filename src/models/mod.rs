pub mod calendar;
pub mod config;
pub mod kanban;
pub mod markdown;
pub mod todo;

pub use calendar::{CalendarData, CalendarEvent};
pub use config::{AppConfig, Note, Tab, TabType};
pub use kanban::{KanbanBoard, KanbanCard, KanbanColumn};
pub use markdown::MarkdownContent;
pub use todo::{TodoItem, TodoList};

#[derive(Debug, Clone, PartialEq)]
pub enum TabContent {
    Markdown(MarkdownContent),
    Kanban(KanbanBoard),
    Todo(TodoList),
    Calendar(CalendarData),
}

impl TabContent {
    pub fn tab_type(&self) -> TabType {
        match self {
            Self::Markdown(_) => TabType::Markdown,
            Self::Kanban(_) => TabType::Kanban,
            Self::Todo(_) => TabType::Todo,
            Self::Calendar(_) => TabType::Calendar,
        }
    }

    pub fn type_name(&self) -> &'static str {
        self.tab_type().as_str()
    }
}
