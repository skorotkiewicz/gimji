use std::fs;

use gimji::models::{CalendarData, KanbanBoard, TabType, TodoList};
use gimji::storage::atomic::atomic_write;
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
fn json_content_schemas_have_no_version_field() {
    let board = KanbanBoard::default();
    let todos = TodoList::default();
    let calendar = CalendarData::default();

    let board_json = serde_json::to_string(&board).unwrap();
    let todos_json = serde_json::to_string(&todos).unwrap();
    let calendar_json = serde_json::to_string(&calendar).unwrap();

    assert!(!board_json.contains("version"));
    assert!(!todos_json.contains("version"));
    assert!(!calendar_json.contains("version"));
}

#[test]
fn atomic_write_replaces_file_contents() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let path = temp_dir.path().join("content.txt");

    atomic_write(&path, b"old").expect("first write");
    atomic_write(&path, b"new").expect("second write");

    assert_eq!(fs::read_to_string(path).expect("read file"), "new");
}
