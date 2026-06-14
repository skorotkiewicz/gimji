use std::fs;

use gimji::models::{
    CalendarData, CalendarEvent, KanbanBoard, KanbanCard, TabContent, TabType, TodoItem, TodoList,
};
use gimji::storage::{DeleteOptions, Workspace};

#[test]
fn workspace_round_trips_metadata_and_tab_content_separately() {
    let temp_dir = tempfile::tempdir().expect("temp workspace");
    let workspace_path = temp_dir.path();

    let mut workspace = Workspace::create(workspace_path).expect("create workspace");
    let note_id = workspace.add_note("Project Notes").expect("add note");
    let markdown_tab_id = workspace
        .selected_tab_id()
        .expect("markdown tab selected")
        .to_owned();
    let kanban_tab_id = workspace
        .add_tab(&note_id, "Board", TabType::Kanban)
        .expect("add kanban tab");
    let todo_tab_id = workspace
        .add_tab(&note_id, "Tasks", TabType::Todo)
        .expect("add todo tab");
    let calendar_tab_id = workspace
        .add_tab(&note_id, "Calendar", TabType::Calendar)
        .expect("add calendar tab");

    workspace
        .save_tab_content(
            &markdown_tab_id,
            &TabContent::Markdown("# Secret markdown body".to_owned()),
        )
        .expect("save markdown");

    let mut board = KanbanBoard::default();
    board.columns[0]
        .cards
        .push(KanbanCard::new("Secret kanban card"));
    workspace
        .save_tab_content(&kanban_tab_id, &TabContent::Kanban(board.clone()))
        .expect("save kanban");

    let mut todos = TodoList::default();
    todos.items.push(TodoItem::new("Secret todo text"));
    workspace
        .save_tab_content(&todo_tab_id, &TabContent::Todo(todos.clone()))
        .expect("save todo");

    let mut calendar = CalendarData::default();
    calendar.events.push(CalendarEvent::new(
        "2026-06-13",
        "Secret event title",
        "Secret event description",
    ));
    workspace
        .save_tab_content(&calendar_tab_id, &TabContent::Calendar(calendar.clone()))
        .expect("save calendar");

    let reloaded = Workspace::open(workspace_path).expect("reload workspace");

    assert_eq!(
        reloaded
            .load_tab_content(&markdown_tab_id)
            .expect("load markdown"),
        TabContent::Markdown("# Secret markdown body".to_owned())
    );
    assert_eq!(
        reloaded
            .load_tab_content(&kanban_tab_id)
            .expect("load kanban"),
        TabContent::Kanban(board)
    );
    assert_eq!(
        reloaded.load_tab_content(&todo_tab_id).expect("load todo"),
        TabContent::Todo(todos)
    );
    assert_eq!(
        reloaded
            .load_tab_content(&calendar_tab_id)
            .expect("load calendar"),
        TabContent::Calendar(calendar)
    );

    let config_text = fs::read_to_string(workspace_path.join("config.json")).expect("read config");
    assert!(config_text.contains("\"file_name\""));
    assert!(!config_text.contains("Secret markdown body"));
    assert!(!config_text.contains("Secret kanban card"));
    assert!(!config_text.contains("Secret todo text"));
    assert!(!config_text.contains("Secret event title"));
    assert!(!config_text.contains("Secret event description"));

    assert!(workspace_path.join("content").is_dir());
    assert_eq!(reloaded.config().notes.len(), 1);
    assert_eq!(reloaded.config().notes[0].tabs.len(), 4);
}

#[test]
fn deleting_note_with_default_options_keeps_local_content_files() {
    let temp_dir = tempfile::tempdir().expect("temp workspace");
    let workspace_path = temp_dir.path();

    let mut workspace = Workspace::create(workspace_path).expect("create workspace");
    let note_id = workspace.add_note("Project Notes").expect("add note");
    let content_file = workspace.config().notes[0].tabs[0].file_name.clone();
    let content_path = workspace_path.join(&content_file);

    assert!(content_path.exists());

    workspace
        .delete_note(&note_id, DeleteOptions::default())
        .expect("delete note");

    assert!(workspace.config().notes.is_empty());
    assert!(content_path.exists());
}

#[test]
fn deleting_note_can_remove_local_content_files_when_requested() {
    let temp_dir = tempfile::tempdir().expect("temp workspace");
    let workspace_path = temp_dir.path();

    let mut workspace = Workspace::create(workspace_path).expect("create workspace");
    let note_id = workspace.add_note("Project Notes").expect("add note");
    workspace
        .add_tab(&note_id, "Tasks", TabType::Todo)
        .expect("add todo tab");

    let content_paths: Vec<_> = workspace.config().notes[0]
        .tabs
        .iter()
        .map(|tab| workspace_path.join(&tab.file_name))
        .collect();

    for path in &content_paths {
        assert!(path.exists());
    }

    workspace
        .delete_note(&note_id, DeleteOptions::remove_local_files())
        .expect("delete note");

    assert!(workspace.config().notes.is_empty());
    for path in &content_paths {
        assert!(!path.exists());
    }
}

#[test]
fn deleting_tab_with_default_options_keeps_local_content_file() {
    let temp_dir = tempfile::tempdir().expect("temp workspace");
    let workspace_path = temp_dir.path();

    let mut workspace = Workspace::create(workspace_path).expect("create workspace");
    let note_id = workspace.add_note("Project Notes").expect("add note");
    let tab_id = workspace
        .add_tab(&note_id, "Tasks", TabType::Todo)
        .expect("add todo tab");
    let content_file = workspace
        .config()
        .notes
        .iter()
        .flat_map(|note| note.tabs.iter())
        .find(|tab| tab.id == tab_id)
        .expect("find tab")
        .file_name
        .clone();
    let content_path = workspace_path.join(&content_file);

    assert!(content_path.exists());

    workspace
        .delete_tab(&tab_id, DeleteOptions::default())
        .expect("delete tab");

    assert_eq!(workspace.config().notes[0].tabs.len(), 1);
    assert!(content_path.exists());
}

#[test]
fn deleting_tab_can_remove_local_content_file_when_requested() {
    let temp_dir = tempfile::tempdir().expect("temp workspace");
    let workspace_path = temp_dir.path();

    let mut workspace = Workspace::create(workspace_path).expect("create workspace");
    let note_id = workspace.add_note("Project Notes").expect("add note");
    let tab_id = workspace
        .add_tab(&note_id, "Tasks", TabType::Todo)
        .expect("add todo tab");
    let content_file = workspace
        .config()
        .notes
        .iter()
        .flat_map(|note| note.tabs.iter())
        .find(|tab| tab.id == tab_id)
        .expect("find tab")
        .file_name
        .clone();
    let content_path = workspace_path.join(&content_file);

    assert!(content_path.exists());

    workspace
        .delete_tab(&tab_id, DeleteOptions::remove_local_files())
        .expect("delete tab");

    assert_eq!(workspace.config().notes[0].tabs.len(), 1);
    assert!(!content_path.exists());
}
