# Workspace Format

This document describes Gimji workspace format version 1.

## Directory Layout

A workspace is a normal directory with these Gimji-managed paths:

- `config.json`: workspace metadata and note/tab index.
- `content/`: tab content files referenced by `config.json`.
- `backups/`: local backup directory reserved by the workspace initializer.
- `.app/`: app-owned local files, including optional S3 settings in `s3.json`.

`.app/s3.json` can contain plaintext S3 credentials. Gimji keeps it local and does
not include it in S3 backups.

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

A Markdown tab can contain multiple files. Its optional `markdown_files` array
stores ordered entries with `id`, `title`, `file_name`, and `collapsed`. Each body
is plain text in a separate `.md` file, never in `config.json`. New files use UUID
filenames; editing a title does not move the file. Preview mode is session-only.

If `markdown_files` is absent, the tab's existing `file_name` and `title` form its
first entry. The array is written on the first edit without moving that file.
Once present, the array is authoritative, including an empty array; the legacy
`file_name` is no longer used. Removing an entry keeps its file unless the user
confirms **Remove local content files**. Backup and restore include all files
under `content/` and validate every active entry's path.

Kanban, todo, and calendar files are JSON data
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
