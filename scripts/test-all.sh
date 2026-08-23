#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
bun run check
bun test tests && (cd sdk/assemblyscript && bun test tests)
./scripts/check-no-x11-features.sh
