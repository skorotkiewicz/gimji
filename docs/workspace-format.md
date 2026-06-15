# Workspace Format

This document describes Gimji workspace format version 1.

## Directory Layout

A workspace is a normal directory with these Gimji-managed paths:

- `config.json`: workspace metadata and note/tab index.
- `content/`: tab content files referenced by `config.json`.
- `backups/`: local backup directory reserved by the workspace initializer.
- `.app/`: app-owned directory reserved by the workspace initializer.

All tab content paths in `config.json` must be relative paths under `content/`.
Absolute paths, parent-directory components, and non-`content/` roots are invalid.

## `config.json`

`config.json` is JSON with `version: 1` and these top-level fields:

- `selected_note_id`: selected note id, or `null`.
- `selected_tab_id`: selected tab id, or `null`.
- `notes`: ordered note list.

Each note has:

- `id`
- `title`
- `created_at`
- `updated_at`
- `tabs`

Each tab has:

- `id`
- `title`
- `type`
- `file_name`
- `created_at`
- `updated_at`

The tab `type` values are `markdown`, `kanban`, `todo`, and `calendar`.

## Content Files

Tab content is stored outside `config.json` in files under `content/`.

Extensions are based on the tab type:

- Markdown tabs use `.md`.
- Kanban tabs use `.kanban.json`.
- Todo tabs use `.todo.json`.
- Calendar tabs use `.calendar.json`.

Markdown files are plain text. Kanban, todo, and calendar files are JSON data
with their own `version: 1` schema version.

## Schema Version

Current schema version values:

- Workspace config: `version: 1`
- Kanban board: `version: 1`
- Todo list: `version: 1`
- Calendar data: `version: 1`

## Migration Rules

Opening a workspace runs migration checks for `config.json` and each typed JSON
content file as it is loaded. Version 1 is accepted. Future versions are rejected
until a migration is added.

Content file paths are validated when a workspace is opened, when content is
loaded, and when content is restored from backup.

## Backup And Restore Guarantees

S3 backup uploads `config.json`, all files under `content/`, and the manifest at
`.gimji/backup-manifest.json`. When an S3 prefix is configured, those object keys
are scoped under the normalized prefix.

The manifest contains the Gimji version, backup timestamp, object list, config
checksum, and content checksums.

S3 restore downloads and validates objects before local writes. It verifies that
the restored config references content files present in the restore payload.
When a manifest is present, restore validates checksums before writing.

S3 restore writes content files before config.json so metadata does not point to
missing or partially restored content after a failed restore.
