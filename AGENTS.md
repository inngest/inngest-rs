# Agent Guide

This file provides guidance to AI coding agents working in this repository.

## Commit Titles

Use conventional commit titles for any commit you create. This repository's changelog configuration in `cliff.toml` enables `conventional_commits = true` and `filter_unconventional = true`, so non-conventional titles are easy to lose or misclassify.

Prefer these commit types because they are the ones grouped or validated in the repo today:

- `feat`
- `fix`
- `doc`
- `perf`
- `refactor`
- `style`
- `test`
- `chore`
- `ci`
- `revert`
- `security`

Use standard conventional commit formatting such as `fix(promapi): handle empty query` or `chore(ci): align changelog workflow permissions`.

## Development Commands

- commit in small logical sections whenever possible to make each commit as reviewable by its own as possible
- run `make lint` and tests targeting the changes to confirm expected results and there are no build failures
- add comments to public structs, traits and functions so it materializes in cargo docs
- if you're referencing a plan while implementing, update the plan's checklists as you go

### Tests

- Always add tests to verify behavior
- When fixing bugs, make sure to create a test case with that ideal outcome that fails with existing behavior before proceeding with a fix

### Building and Running

- `cargo check -p inngest` checks the crate without running tests
- `cargo build -p inngest` builds the crate
- `make dev` runs the `axum` example via `cargo run -p inngest --example axum`
- `make inngest-dev` starts `inngest` against `http://127.0.0.1:3000/api/inngest`

### Testing and Quality

- `make test` or `cargo test` runs the full test suite
- `cargo test -p inngest` runs tests for the SDK crate
- `cargo fmt --check` matches the CI formatting check
- `make fmt` runs `cargo fmt`
- `make lint` runs `cargo clippy`
- `cargo clippy --all-features` matches the CI lint configuration

## Working Style

- Prefer minimal, targeted changes that preserve the existing code style.
- Run relevant tests for the area you touch when practical.
- Do not introduce a second commit-title convention; keep commit types aligned with `cliff.toml` and `.github/workflows/commits.yml`.
