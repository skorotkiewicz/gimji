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
    #[serde(default)]
    pub tags: Vec<String>,
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
            tags: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = timestamp();
    }
}

#[cfg(test)]
mod tests {
    use super::TodoItem;

    #[test]
    fn legacy_todo_without_tags_loads_with_empty_tags() {
        let item: TodoItem = serde_json::from_str(
            r#"{"id":"todo-1","text":"Ship it","done":false,"created_at":"2026-07-01","updated_at":"2026-07-01"}"#,
        )
        .expect("legacy todo item");

        assert!(item.tags.is_empty());
    }
}
