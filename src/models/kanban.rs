use serde::{Deserialize, Serialize};

use crate::models::config::{new_id, timestamp};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KanbanBoard {
    pub columns: Vec<KanbanColumn>,
}

impl Default for KanbanBoard {
    fn default() -> Self {
        Self {
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
    #[serde(default, alias = "text")]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub details_hidden: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl KanbanCard {
    pub fn new(name: impl Into<String>) -> Self {
        let now = timestamp();

        Self {
            id: new_id(),
            name: name.into(),
            description: String::new(),
            details_hidden: false,
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
    use super::KanbanCard;

    #[test]
    fn card_reads_legacy_text_as_name() {
        let card: KanbanCard = serde_json::from_str(
            r#"{"id":"card-1","text":"Ship it","created_at":"2026-07-01","updated_at":"2026-07-01"}"#,
        )
        .expect("legacy kanban card");

        assert_eq!(card.name, "Ship it");
        assert_eq!(card.description, "");
        assert!(!card.details_hidden);
    }
}
