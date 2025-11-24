# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Workflow

1. **Code Formatting**: Always run `cargo +nightly fmt` after making code changes
2. **Code Linting**: Always run `cargo clippy` after making code changes. If clippy warnings are simple, fix them. If complicated, ask the user for guidance.
3. **CRITICAL: Format and Lint Before Any Commit**: Always run both `cargo +nightly fmt` and `cargo clippy` before committing changes. This is mandatory for all code changes.
4. **Comments**: Do not add trivial, obvious, or redundant comments. Only include comments that explain complex logic, business rules, or non-obvious behavior. Avoid comments like `// Create connection`, `// Set to true`, or `// 30 seconds` that simply restate what the code does.
5. **Commit Messages**: Do not add Gemini attribution or co-authorship to commit messages. Keep commit messages clean and professional.
6. **Code Consistency**: Be consistent with existing code patterns, naming conventions, and architectural decisions in the codebase.

## Rust Configuration

- **Rust version**: 1.89.0 (specified in `rust-toolchain.toml`)
- **Edition**: 2024
- **Key features**: async/await with tokio, egui for GUI
