use serde::{Deserialize, Serialize};

use crate::models::config::{new_id, timestamp};

pub const KANBAN_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KanbanBoard {
    pub version: u32,
    pub columns: Vec<KanbanColumn>,
}

impl Default for KanbanBoard {
    fn default() -> Self {
        Self {
            version: KANBAN_VERSION,
            columns: vec![
                KanbanColumn::new("todo", "Todo"),
                KanbanColumn::new("doing", "Doing"),
                KanbanColumn::new("done", "Done"),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KanbanColumn {
    pub id: String,
    pub title: String,
    pub cards: Vec<KanbanCard>,
}

impl KanbanColumn {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            cards: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KanbanCard {
    pub id: String,
    pub text: String,
    pub created_at: String,
    pub updated_at: String,
}

impl KanbanCard {
    pub fn new(text: impl Into<String>) -> Self {
        let now = timestamp();

        Self {
            id: new_id(),
            text: text.into(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = timestamp();
    }
}
