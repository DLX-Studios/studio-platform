#!/usr/bin/env bash
set -euo pipefail

cargo build --manifest-path fuzz/Cargo.toml --bins >/dev/null
for target in protocol_decode patch_transaction bundle_parse action_payload; do
  log_file=$(mktemp "${TMPDIR:-/tmp}/studio-fuzz-${target}.XXXXXX")
  "fuzz/target/debug/${target}" "fuzz/corpus/${target}" \
    -max_total_time=60 -print_final_stats=1 >"${log_file}" 2>&1
  executed=$(awk '/stat::number_of_executed_units:/{print $2}' "${log_file}")
  if [[ -z "${executed}" || "${executed}" == "0" ]]; then
    echo "${target}: no fuzz executions recorded" >&2
    sed -n '1,160p' "${log_file}" >&2
    rm -f -- "${log_file}"
    exit 1
  fi
  echo "${target}: 60-second smoke passed (${executed} executions)"
  rm -f -- "${log_file}"
done
