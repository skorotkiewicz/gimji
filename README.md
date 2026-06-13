# Gimji

Gimji is a local-first Rust desktop notes app built with `eframe`/`egui`.

## Run

```bash
cargo run
```

## Verify

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

## Workspace Format

Open or create a folder from the app. Gimji stores note and tab metadata in
`config.json`, and stores user content in separate files under `content/`.

```text
workspace/
  config.json
  content/
    project-markdown-abc12345.md
    project-board-def67890.kanban.json
    project-tasks-ghi12345.todo.json
    project-calendar-jkl67890.calendar.json
  backups/
  .app/
```

`config.json` is metadata only. Markdown text, kanban cards, todo items, and
calendar events are saved in their own content files.
