#!/usr/bin/env bash
set -euo pipefail

started_seconds=$(date +%s)
bun install --frozen-lockfile
bun run build:starter
bun test tests/e2e/starter_plugin.test.ts
cargo test -p studio-app --test developer_errors
elapsed_seconds=$(( $(date +%s) - started_seconds ))

if (( elapsed_seconds >= 600 )); then
  echo "starter quickstart exceeded ten minutes (${elapsed_seconds}s)" >&2
  exit 1
fi
echo "starter quickstart completed in ${elapsed_seconds}s (under ten minutes)"
