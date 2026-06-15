# Gimji

Minimal local-first notes for projects, tasks, boards, calendars, and markdown.
Gimji stores everything in a folder you choose, so your workspace stays plain,
portable, and easy to back up.

## Quick Start

```bash
cargo run
```

Open or create a workspace folder from the sidebar. Add notes, add tabs, and
edit content directly. Gimji saves metadata in `config.json` and tab content in
separate files under `content/`.

## Arch Linux

Gimji is available from the AUR as `gimji`.

```bash
paru -S gimji
# or
yay -S gimji
```

## Build

```bash
cargo build --release
```

The binary is written to `target/release/gimji`.

## S3 Backups

S3 support is optional. Build with all features to enable it:

```bash
cargo build --release --all-features
```

You can pre-fill the S3 form with environment variables:

```bash
export GIMJI_S3_BUCKET=storage
export GIMJI_S3_REGION=us-east-1
export GIMJI_S3_ENDPOINT=http://127.0.0.1:9000
export GIMJI_S3_PREFIX=gimji1
export GIMJI_S3_ACCESS_KEY=minioadmin
export GIMJI_S3_SECRET_KEY=minioadmin
```

Open the S3 section in the sidebar, test the connection, then use `Backup` or
`Restore`.

Optional in `GIMJI_S3_PREFIX` use a different prefix, such as `gimji1`, `gimji2`, or a project name, to keep multiple Gimji workspaces separate while sharing one S3 bucket.

## Verify

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```
