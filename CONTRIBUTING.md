# Contributing to Gimji

Thanks for your interest in contributing to Gimji! This document outlines the process for contributing to the project.

## Reporting Bugs

1. Check if the bug has already been reported in [Issues](https://github.com/skorotkiewicz/gimji/issues)
2. If not, create a new issue with:
   - A clear, descriptive title
   - Steps to reproduce the bug
   - Expected vs actual behavior
   - Gimji version, OS, and desktop environment if relevant
   - Screenshots if applicable

## Suggesting Features

Open an issue with the `enhancement` label describing:
- The problem you're trying to solve
- Your proposed solution
- Any alternatives you've considered

## Pull Requests

### Before You Start

- For small fixes (typos, minor bugs), feel free to submit a PR directly
- For larger changes, open an issue first to discuss the approach

### Development Setup

1. Fork and clone the repository
2. Run the app:
   ```bash
   cargo run
   ```
3. Run with optional S3 support if needed:
   ```bash
   cargo run --all-features
   ```

### Making Changes

1. Create a branch from `main`:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. Make your changes following the code style below

3. Test your changes:
   ```bash
   just fmt && cargo test
   ```

4. Commit with a clear message:
   ```bash
   git commit -m "Add feature X that does Y"
   ```

### Code Style

- Use `cargo fmt`; do not hand-format Rust.
- Keep changes small and focused.
- Prefer existing modules and patterns.
- Avoid new dependencies unless needed.
- Add one focused test for non-trivial behavior.

### Submitting

1. Push to your fork
2. Open a Pull Request
3. Wait for review

## Code of Conduct

Be respectful and constructive. See `CODE_OF_CONDUCT.md`.

## Questions?

Open an issue with the `question` label or reach out to the maintainers.
