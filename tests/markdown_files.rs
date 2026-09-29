use std::fs;

use gimji::models::{MarkdownDocument, TabType};
use gimji::storage::{DeleteOptions, Workspace};

#[test]
fn legacy_markdown_becomes_independent_files_and_can_be_removed_to_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mut workspace = Workspace::create(dir.path()).unwrap();
    workspace.add_note("Notes").unwrap();
    let tab_id = workspace.selected_tab_id().unwrap().to_owned();
    workspace
        .save_markdown_content(&tab_id, "# Original body")
        .unwrap();
    let original_path = workspace.find_tab(&tab_id).unwrap().file_name.clone();
    assert!(
        !fs::read_to_string(dir.path().join("config.json"))
            .unwrap()
            .contains("markdown_files")
    );

    let mut documents = workspace.load_markdown_documents(&tab_id).unwrap();
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].text, "# Original body");
    assert_eq!(documents[0].file.file_name, original_path);
    documents[0].file.title = "Renamed / title".to_owned();
    documents[0].file.collapsed = true;
    let mut second = MarkdownDocument::new("Second");
    second.text = "独立\n\n```rust\nfn main() {}\n```\n".to_owned();
    let second_path = second.file.file_name.clone();
    documents.push(second);
    workspace
        .save_markdown_documents(&tab_id, &documents)
        .unwrap();

    let mut workspace = Workspace::open(dir.path()).unwrap();
    assert_eq!(
        workspace.load_markdown_documents(&tab_id).unwrap(),
        documents
    );
    assert_eq!(
        fs::read_to_string(dir.path().join(&original_path)).unwrap(),
        documents[0].text
    );
    assert_eq!(
        fs::read_to_string(dir.path().join(&second_path)).unwrap(),
        documents[1].text
    );
    let config = fs::read_to_string(dir.path().join("config.json")).unwrap();
    assert!(!config.contains("Original body"));
    assert!(!config.contains("独立"));

    workspace
        .remove_markdown_document(&tab_id, &documents[0].file.id, DeleteOptions::default())
        .unwrap();
    assert!(dir.path().join(&original_path).exists());
    assert_eq!(
        workspace.load_markdown_content(&tab_id).unwrap(),
        documents[1].text
    );
    workspace
        .remove_markdown_document(
            &tab_id,
            &documents[1].file.id,
            DeleteOptions::remove_local_files(),
        )
        .unwrap();
    assert!(!dir.path().join(second_path).exists());
    let workspace = Workspace::open(dir.path()).unwrap();
    assert!(
        workspace
            .load_markdown_documents(&tab_id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        workspace.find_tab(&tab_id).unwrap().content_files().count(),
        0
    );
}

#[test]
fn markdown_paths_and_failed_metadata_writes_do_not_destroy_content() {
    let dir = tempfile::tempdir().unwrap();
    let mut workspace = Workspace::create(dir.path()).unwrap();
    workspace.add_note("Notes").unwrap();
    let tab_id = workspace.selected_tab_id().unwrap().to_owned();
    workspace.save_markdown_content(&tab_id, "keep me").unwrap();
    let mut documents = workspace.load_markdown_documents(&tab_id).unwrap();
    documents[0].text = "edited".to_owned();
    let mut invalid = MarkdownDocument::new("Invalid");
    invalid.file.file_name = "content/../../outside.md".to_owned();
    documents.push(invalid);
    assert!(
        workspace
            .save_markdown_documents(&tab_id, &documents)
            .is_err()
    );
    assert_eq!(workspace.load_markdown_content(&tab_id).unwrap(), "keep me");

    documents.pop();
    let old_config = workspace.config().clone();
    fs::create_dir(dir.path().join("config.tmp")).unwrap();
    assert!(
        workspace
            .remove_markdown_document(
                &tab_id,
                &documents[0].file.id,
                DeleteOptions::remove_local_files()
            )
            .is_err()
    );
    assert_eq!(workspace.config(), &old_config);
    assert_eq!(workspace.load_markdown_content(&tab_id).unwrap(), "keep me");
    documents.push(MarkdownDocument::new("New file"));
    assert!(
        workspace
            .save_markdown_documents(&tab_id, &documents)
            .is_err()
    );
    assert_eq!(workspace.config(), &old_config);
    assert_eq!(Workspace::open(dir.path()).unwrap().config(), &old_config);

    // Every entry is validated on open, not just the legacy tab path.
    let mut config = serde_json::to_value(old_config).unwrap();
    config["notes"][0]["tabs"][0]["markdown_files"] = serde_json::json!([{
        "id": "bad", "title": "Bad", "file_name": "../outside.md"
    }]);
    fs::write(
        dir.path().join("config.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    assert!(Workspace::open(dir.path()).is_err());
}

#[test]
fn deleting_a_tab_or_note_includes_every_markdown_file() {
    for delete_note in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut workspace = Workspace::create(dir.path()).unwrap();
        let note_id = workspace.add_note("Notes").unwrap();
        let tab_id = workspace.selected_tab_id().unwrap().to_owned();
        workspace.add_tab(&note_id, "Keep", TabType::Todo).unwrap();
        let mut documents = workspace.load_markdown_documents(&tab_id).unwrap();
        documents.push(MarkdownDocument::new("Another file"));
        workspace
            .save_markdown_documents(&tab_id, &documents)
            .unwrap();
        if delete_note {
            workspace
                .delete_note(&note_id, DeleteOptions::remove_local_files())
                .unwrap();
        } else {
            workspace
                .delete_tab(&tab_id, DeleteOptions::remove_local_files())
                .unwrap();
        }
        for document in documents {
            assert!(!dir.path().join(document.file.file_name).exists());
        }
    }
}
