use std::fs;

use gimji::models::{CalendarData, KanbanBoard, TabType, TodoList};
use gimji::storage::atomic::atomic_write;
use gimji::storage::migration::{migrate_calendar, migrate_kanban, migrate_todo};
use gimji::storage::{make_content_file_name, sanitize_file_stem, validate_relative_content_path};

#[test]
fn generated_content_file_names_are_sanitized() {
    assert_eq!(sanitize_file_stem(" Project: Notes! "), "project-notes");
    assert_eq!(sanitize_file_stem("../"), "file");

    let file_name = make_content_file_name(
        "Project: Notes",
        "Board/Today",
        TabType::Kanban,
        "abcdef12-3456",
    );

    assert_eq!(
        file_name,
        "content/project-notes-board-today-abcdef12.kanban.json"
    );
}

#[test]
fn content_paths_cannot_escape_workspace_content_directory() {
    assert!(validate_relative_content_path("content/project.md").is_ok());
    assert!(validate_relative_content_path("content/nested/project.md").is_ok());
    assert!(validate_relative_content_path("../config.json").is_err());
    assert!(validate_relative_content_path("content/../config.json").is_err());
    assert!(validate_relative_content_path("/tmp/project.md").is_err());
    assert!(validate_relative_content_path("backups/project.md").is_err());
}

#[test]
fn json_content_schemas_are_versioned() {
    let board = KanbanBoard::default();
    let todos = TodoList::default();
    let calendar = CalendarData::default();

    assert_eq!(board.version, 1);
    assert_eq!(todos.version, 1);
    assert_eq!(calendar.version, 1);

    assert!(
        serde_json::to_string(&board)
            .unwrap()
            .contains("\"version\":1")
    );
    assert!(
        serde_json::to_string(&todos)
            .unwrap()
            .contains("\"version\":1")
    );
    assert!(
        serde_json::to_string(&calendar)
            .unwrap()
            .contains("\"version\":1")
    );
}

#[test]
fn migration_stubs_accept_version_one_and_reject_future_versions() {
    assert!(migrate_kanban(KanbanBoard::default()).is_ok());
    assert!(migrate_todo(TodoList::default()).is_ok());
    assert!(migrate_calendar(CalendarData::default()).is_ok());

    let mut future_board = KanbanBoard::default();
    future_board.version = 2;
    assert!(migrate_kanban(future_board).is_err());
}

#[test]
fn atomic_write_replaces_file_contents() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let path = temp_dir.path().join("content.txt");

    atomic_write(&path, b"old").expect("first write");
    atomic_write(&path, b"new").expect("second write");

    assert_eq!(fs::read_to_string(path).expect("read file"), "new");
}
