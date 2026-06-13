use crate::Result;
use crate::errors::AppError;
use crate::models::config::CONFIG_VERSION;
use crate::models::{AppConfig, CalendarData, KanbanBoard, TodoList};

pub fn migrate_config(config: &mut AppConfig) -> Result<()> {
    match config.version {
        CONFIG_VERSION => Ok(()),
        version => Err(AppError::UnsupportedVersion {
            kind: "config",
            version,
        }),
    }
}

pub fn migrate_kanban(board: KanbanBoard) -> Result<KanbanBoard> {
    match board.version {
        crate::models::kanban::KANBAN_VERSION => Ok(board),
        version => Err(AppError::UnsupportedVersion {
            kind: "kanban",
            version,
        }),
    }
}

pub fn migrate_todo(list: TodoList) -> Result<TodoList> {
    match list.version {
        crate::models::todo::TODO_VERSION => Ok(list),
        version => Err(AppError::UnsupportedVersion {
            kind: "todo",
            version,
        }),
    }
}

pub fn migrate_calendar(calendar: CalendarData) -> Result<CalendarData> {
    match calendar.version {
        crate::models::calendar::CALENDAR_VERSION => Ok(calendar),
        version => Err(AppError::UnsupportedVersion {
            kind: "calendar",
            version,
        }),
    }
}
