use serde::{Deserialize, Serialize};

use crate::models::config::{new_id, timestamp};

pub const TODO_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TodoList {
    pub version: u32,
    pub items: Vec<TodoItem>,
}

impl Default for TodoList {
    fn default() -> Self {
        Self {
            version: TODO_VERSION,
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TodoItem {
    pub id: String,
    pub text: String,
    pub done: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl TodoItem {
    pub fn new(text: impl Into<String>) -> Self {
        let now = timestamp();

        Self {
            id: new_id(),
            text: text.into(),
            done: false,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = timestamp();
    }
}
