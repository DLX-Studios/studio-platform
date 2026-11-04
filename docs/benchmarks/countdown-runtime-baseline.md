# Countdown runtime baseline

Baseline date: 2026-11-04. Release build and measurements completed.

Purpose: compare the existing AssemblyScript/Wasm release runtime against a future
Studio IR interpreter with host-owned reactivity. No deployment is involved.

## Conditions

- Linux x86_64; AMD Ryzen 7 6800H, 16 logical CPUs, approximately 31 GiB RAM.
- Current working tree, based on commit e05de5b61525c2c780bfe26cd629c98e45354202;
  existing uncommitted routing/runtime work is included.
- `cargo build --locked --release -p studio-app`; shared Cargo target, no clean.
- `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0`.
- Isolated headless Sway at 1440 × 900, scale 1, pixman software compositor; `LIBGL_ALWAYS_SOFTWARE=1`.
  The GPUI Vulkan adapter is not pinned or conclusively identified.
- Real release executable, signed countdown bundle, isolated trust for the
  repository's public development signing seed. No trust checks bypassed.
- Original generated 1672 × 941 PNG cityscape, ten-minute countdown unchanged.
- User-requested package-policy change included: assets total 256 MiB (formerly
  1 MiB), encoded archive 512 MiB (formerly 16 MiB). A single asset is bounded by
  the aggregate ceiling. Module and entry-count limits are unchanged.
- These are encoded asset sizes, not decoded image or GPU-memory guarantees.
  An 7680 × 4320 RGBA8 surface alone occupies approximately 126.6 MiB.

## Reproduction

Build runtime and CLI sequentially with other Cargo operations. Build the bundle
using `studio build examples/stream-countdown`. Then:

```sh
python3 scripts/benchmark-countdown.py \
  --binary /var/home/swhite/Development/.cargo-target/release/studio-app \
  --bundle examples/stream-countdown/build/stream-countdown.studio \
  --output .scratch/runtime-baseline/run --seconds 615
```

Harness dependencies are Python 3, Pillow, cryptography, Sway, grim and dbus-run-session.
These are benchmark dependencies, not proposed app-author requirements.

Three launches are measured; the first has uncontrolled filesystem/driver caches,
and the next two are warm. Screenshot polling estimates time to visible textured
content, including screenshot overhead; it is not an instrumented first-frame timestamp.
The third launch runs for 615 seconds, sampling process RSS, peak RSS, PSS, thread
count and CPU once per second. CPU uses 100% for one fully utilized core. Sampling
and screenshot processes are outside the app's CPU measurement but can compete for
resources. Rendering work may execute in the app process; loaded Vulkan drivers alone do
not establish which adapter rendered the app. Compositor memory and
GPU allocations are not comprehensively attributed by process RSS/PSS.

## Comparison requirements

Use the same machine, display, assets, countdown duration, release settings and
workload after migration. Compare binary and bundle sizes, startup samples, idle/live
and countdown CPU, RSS/PSS peaks, and memory after repeated route/reset interactions.
Keep compiler build time separate from app packaging time and app startup time.
Record background load and cache conditions. Do not label these launches cold starts.

The countdown is a small compatibility workload, not a large-list or compute benchmark.
A second workload is needed before generalizing throughput to complex apps.

## Initial build and startup evidence

- Initial cached/incremental dependency rebuild: 255.19 s wall time.
- Follow-up release build of runtime and CLI after the asset-policy edit: 34.24 s wall time
  (reported Cargo time 34.14 s).
- Successful app packaging: 0.49 s wall time. The earlier 1 MiB-policy packaging
  attempt failed and is excluded from successful packaging timing.
- Final runtime: 98,369,984 bytes (93.81 MiB).
- Signed countdown bundle: 2,707,070 bytes (2.58 MiB).
- First observed content: 1.062 s; warm observations: 0.642 s and 0.640 s.
- Binary SHA-256: `8335f57ff92439f6374e4c2a213a988f7c7798ed203b8c692d3dc7134ae7e5bc`.
- Bundle SHA-256: `e65eb4e8ffcbeb5d828773003ece94788a6d01545738bf39845ca0b08969422c`.

The first screenshot confirms the original cityscape and 10:00 countdown rendered.
The known white rectangle behind the title remains; no renderer fix is included.

Raw evidence is in `.scratch/runtime-baseline/`: build logs and `/usr/bin/time -v`
reports, source hashes, and `run/` containing results JSON, per-second CSV samples,
screenshots, and runtime/compositor logs. The scratch directory is local and ignored;
summary data will be copied into this document for durable review.

Memory terminology: RSS (Resident Set Size) counts process pages resident in physical
RAM, including shared pages in full. PSS (Proportional Set Size) apportions shared
pages among their users. Neither equals executable size, and neither fully reports
GPU-memory usage. `VmHWM` is the kernel's process RSS high-water mark.

## Completed baseline results

The full run completed successfully: 616 samples over 615.007 seconds. Screenshots
show 10:00 initially, 9:00 at one minute, 5:00 at five minutes, the clock hidden at
600 seconds, and the live screen at 601 seconds. This brackets the transition;
it does not measure the exact fade duration. Runtime stderr was empty.

| Metric | Result |
| --- | ---: |
| Settled countdown median RSS | 324.38 MiB |
| Settled countdown median PSS | 242.31 MiB |
| Process peak RSS | 327.42 MiB |
| Highest sampled PSS | 244.69 MiB |
| Early versus late countdown median RSS increase | 0.10 MiB |
| Countdown CPU, 10–590 s | 115.40% of one core |
| Live idle CPU, 605–615 s | 0.10% of one core |
| Live idle median RSS / PSS | 327.42 / 240.40 MiB |
| Maximum sampled thread count | 93 |

CPU is process-wide, including renderer/driver threads. These figures do not isolate
Wasm execution from rendering. High countdown CPU and very low live-screen CPU
motivate profiling active updates and animations; they do not establish their exact
cause. RSS remaining after navigation can reflect retained pages, caches or allocator
retention; it is not by itself evidence of a leak.

A separate run completed 20 countdown → live → countdown cycles with reset clicks.
Screenshots confirm the live route and final 10:00 state. RSS after each cycle ranged
from 327.270 to 327.281 MiB (12 KiB variation). Median observed route-change time was
315.5 ms, including helper startup, 120 ms of deliberate input synchronization and
screenshot overhead. This is an end-to-end automation observation, not intrinsic
handler latency. An earlier attempt using Sway cursor commands failed to deliver
input; those failed measurements are excluded.

Durable raw measurements: [baseline data](countdown-baseline-2026-11-04/summary.json),
[per-second samples](countdown-baseline-2026-11-04/samples.csv),
[startup and binary identity](countdown-baseline-2026-11-04/results.json),
[interaction measurements](countdown-baseline-2026-11-04/interactions.json).

Verification: all five `studio-package` archive-security tests passed, including
large-asset round-trip and custom individual/aggregate limits. Focused Clippy
(`cargo clippy --locked -p studio-package --test archive_security -- -D warnings`),
format checks on the changed Rust files, and `git diff --check` passed. The benchmark does
not prove decoding every 8K image format or running large application workloads.

## Repeat interaction measurements

Generate `virtual-pointer.h` and `virtual-pointer.c` with `wayland-scanner` from
`wayland-protocols-wlr`'s `wlr-virtual-pointer-unstable-v1.xml` (the Cargo registry
contains this protocol). Compile the local helper:

```sh
gcc -Wall -Wextra -I.scratch/runtime-baseline \
  scripts/benchmark-pointer.c .scratch/runtime-baseline/virtual-pointer.c \
  -lwayland-client -o .scratch/runtime-baseline/benchmark-pointer
python3 scripts/benchmark-countdown.py \
  --binary /var/home/swhite/Development/.cargo-target/release/studio-app \
  --bundle examples/stream-countdown/build/stream-countdown.studio \
  --output .scratch/runtime-baseline/interactions-repeat --seconds 5 --cycles 20 \
  --pointer .scratch/runtime-baseline/benchmark-pointer
```

Coordinates are tied to this fixture and 1440 × 900 layout. Keep the same layout
for comparisons; inspect screenshots to confirm the intended controls were used.
All benchmark-owned app and compositor processes are stopped by the harness.

## Shared-runtime investigation

A future single process can own multiple app windows, separate per-app state and
reactive graphs, and shared renderer, fonts, caches and execution-engine resources.
This is a separate experiment, not implemented in this baseline. Compare empty host,
one app, two apps in one host, and two separate processes. Use aggregate PSS/private
memory rather than simply adding RSS, because OS-shared pages appear in each RSS.
Measure closing one app and verify its subscriptions, timers and resources are released.
A process-wide crash remains a shared failure domain; app-local error handling does
not isolate native crashes or out-of-memory failures. Optional worker processes can
provide stronger fault isolation at a memory/IPC cost. Profile resource ownership
before removing product features based solely on total process memory.


## Two separate instances: 30-second check

Two release runtime processes were launched concurrently using the same binary and
bundle hashes. A 2880 × 900 headless output kept both windows visible, each 1440 × 900;
window geometry is preserved in `countdown-baseline-2026-11-04/pair-window-tree.json`.
Both screenshots showed advancing countdowns. Each process was sampled once per
second through 30 seconds from launch, then both processes and the compositor stopped.
No runtime code or app assets changed for this test.

Median memory for the 10–30 second settled interval:

| Process | RSS | PSS |
| --- | ---: | ---: |
| Instance 1 | 322.25 MiB | 198.81 MiB |
| Instance 2 | 311.91 MiB | 188.42 MiB |
| Combined | 634.16 MiB | 387.23 MiB |

The original single-process run's matching 10–30 second interval had 242.22 MiB
median PSS. The pair's combined PSS is therefore approximately 145.01 MiB higher.
This is a comparison between runs, not a same-process before/after allocation trace.
PSS accounts for sharing that summed RSS double-counts. The two processes still
have separate app state and execution/renderer instances; this does not measure a
future shared-runtime implementation. Compositor and unaccounted GPU memory are
excluded. This short run does not establish long-term memory behavior.

Reproduce with `scripts/benchmark-countdown-pair.py`, using the same `--binary`,
`--bundle`, and a fresh `--output` directory, plus `--seconds 30`.
Raw data: [pair results](countdown-baseline-2026-11-04/pair-results.json) and
[pair samples](countdown-baseline-2026-11-04/pair-samples.csv).
