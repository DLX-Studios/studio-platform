# AGENTS.md

## Commands
- `cargo test --locked --workspace` - run the full test suite
- `cargo clippy --locked --workspace --all-targets -- -D warnings` - run the lint task
- `cargo fmt --all -- --check` - run the format task
- `cargo build --locked --release -p studio-app` - build the project

## Build and Verification Policy
- During ticket work, run focused package, library, and named-test commands for the code in scope.
- Reserve full workspace tests and `cargo clippy --locked --workspace --all-targets -- -D warnings` for integration checkpoints.
- At an integration checkpoint, run format, workspace tests, and Clippy sequentially with one active Cargo process. Run a release build only at an explicit release checkpoint.
- Bind each `CARGO_TARGET_DIR` cache to one exact commit and build configuration; serialize every command that uses it.
- For verification builds that do not require debugger symbols or incremental reuse, set `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, and `CARGO_INCREMENTAL=0` before the first command so the cache keeps one consistent configuration.
- Reuse focused test evidence within a ticket pass instead of rebuilding every integration-test executable.

## Code Map
- `crates` - crates
- `tests` - automated tests
- `docs` - project documentation
- `.github` - project configuration

## Conventions
- Use `use` and `pub mod` in Rust source.
- Use `import` and `export` in TypeScript source.
- Require `.rs` for Rust and `.ts` for TypeScript files.
- Name tests as `*.test.ts`.
