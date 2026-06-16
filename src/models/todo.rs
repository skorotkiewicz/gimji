use serde::{Deserialize, Serialize};

use crate::models::config::{new_id, timestamp};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TodoList {
    pub items: Vec<TodoItem>,
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
