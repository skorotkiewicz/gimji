use serde::{Deserialize, Serialize};

use crate::models::config::{new_id, timestamp};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CalendarData {
    pub events: Vec<CalendarEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalendarEvent {
    pub id: String,
    pub date: String,
    pub title: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

impl CalendarEvent {
    pub fn new(
        date: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        let now = timestamp();

        Self {
            id: new_id(),
            date: date.into(),
            title: title.into(),
            description: description.into(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = timestamp();
    }
}
