# AGENTS.md

## Commands
- `cargo test --locked --workspace` - run the full test suite
- `cargo clippy --locked --workspace --all-targets -- -D warnings` - run the lint task
- `cargo fmt --all -- --check` - run the format task
- `cargo build --locked --release -p studio-app` - build the project

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
