#!/usr/bin/env bash
set -euo pipefail

bun run build:pos >/dev/null
output=$(cargo bench -p studio-app --bench acceptance 2>&1)
echo "${output}"
grep -q '^STUDIO-BENCH-1$' <<<"${output}"

launch_us=$(awk -F= '/^warm_first_frame_p50_us=/{print $2}' <<<"${output}")
interaction_us=$(awk -F= '/^interaction_p95_us=/{print $2}' <<<"${output}")
single_property_us=$(awk -F= '/^single_property_p95_us=/{print $2}' <<<"${output}")
property_batch_us=$(awk -F= '/^property_batch_100_p95_us=/{print $2}' <<<"${output}")
transition_sample_us=$(awk -F= '/^transition_sample_p95_us=/{print $2}' <<<"${output}")
idle_work=$(awk -F= '/^idle_repeated_work=/{print $2}' <<<"${output}")
if [[ -z "${launch_us}" || -z "${interaction_us}" || -z "${single_property_us}" || \
      -z "${property_batch_us}" || -z "${transition_sample_us}" || -z "${idle_work}" ]]; then
  echo "acceptance benchmark output incomplete" >&2
  exit 1
fi
if (( launch_us > 150000 )); then
  echo "warm first frame exceeded 150 ms: ${launch_us} us" >&2
  exit 1
fi
if (( interaction_us > 100000 )); then
  echo "interaction p95 exceeded 100 ms: ${interaction_us} us" >&2
  exit 1
fi
if (( single_property_us > 2000 )); then
  echo "single-property p95 exceeded 2 ms: ${single_property_us} us" >&2
  exit 1
fi
if (( property_batch_us > 8000 )); then
  echo "100-property batch p95 exceeded 8 ms: ${property_batch_us} us" >&2
  exit 1
fi
if (( transition_sample_us > 16667 )); then
  echo "transition sampling exceeded one 60 FPS frame: ${transition_sample_us} us" >&2
  exit 1
fi
if (( idle_work != 0 )); then
  echo "idle plugin produced repeated work" >&2
  exit 1
fi
