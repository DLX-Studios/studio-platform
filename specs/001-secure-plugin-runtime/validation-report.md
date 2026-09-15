# Milestone-One Validation Report

**Validation date:** 2026-08-04
**Result:** T103 automated validation passes after the full `gpui-component` control integration;
the formal low-power baseline is intentionally deferred, and human release gates remain.

## Validation Host

This run is informative because it is not the required `STUDIO-BENCH-1` release device.

| Field | Measured value |
| --- | --- |
| CPU | AMD Ryzen 5 7600X, 6 cores / 12 threads |
| Memory | 14 GiB RAM, 4 GiB swap |
| GPU | AMD Radeon RX 5700 XT (RADV NAVI10) |
| Kernel | Linux 6.18.4-zen1-1-zen x86_64 |
| Vulkan / Mesa | Vulkan 1.4.335; Mesa RADV 25.3.3-arch1.2 |
| Wayland | wayland-client 1.24.0 |
| Compositor | Sway 1.11, headless backend, XWayland disabled |
| Output | Headless virtual output; no physical resolution was asserted |
| Rust | rustc 1.96.0-nightly (d9563937f 2026-03-03) |
| Bun | 1.3.9 |
| Build profile | Rust release for release tests/platform checks/benchmarks |

The originally specified `STUDIO-BENCH-1` is an Intel Processor N100 with 8 GiB RAM, integrated
Intel UHD graphics, NVMe storage, 1920x1080@60 output, and native Weston. Per the current release
decision, that low-power POS run is deferred and is not treated as a completed release gate.
Performance results below are informative results from the validation host.

## Automated Results

| Gate | Result | Evidence |
| --- | --- | --- |
| Repository verification | PASS | `bun run test:all`; all Rust checks and 49 Bun tests passed |
| Release Rust workspace | PASS | `cargo test --locked --workspace --release`; 60 test binaries/doc-test groups passed |
| Generated contracts | PASS | generation completed and the schema/AssemblyScript paths had no tracked drift |
| Deterministic POS packaging | PASS | signed example built and explicit unsigned development bundle was produced |
| Production/development launch policy | PASS | both focused `plugin_launch` tests passed |
| Starter author flow | PASS | completed in 2 seconds, below the ten-minute limit |
| No-X11 Cargo feature graph | PASS | no X11 capability detected |
| Release linkage | PASS | no `libX11` or `libxcb` linkage detected |
| Unsupported display behavior | PASS | missing Wayland endpoint rejected with controlled exit |
| Native compositor launch | PASS | headless Sway launch succeeded and started no XWayland process |
| Headless cleanup | PASS | the isolated compositor process group exited without leaving a Sway process behind |
| Traceability | PASS | all 44 FR/SC identifiers map exactly once to evidence |

The four bounded 60-second fuzz runs passed without a crash:

| Target | Executions |
| --- | ---: |
| `protocol_decode` | 39,223,357 |
| `patch_transaction` | 24,644,512 |
| `bundle_parse` | 39,503,312 |
| `action_payload` | 58,084,612 |

The acceptance benchmark used 5 launch warm-ups plus 30 measured launches and 10 interaction
warm-ups plus 100 measured operations:

| Metric | Result | Limit | Status on this host |
| --- | ---: | ---: | --- |
| Warm first frame p50 | 19,758 us | 150,000 us | PASS |
| Interaction p95 | 205 us | 100,000 us | PASS |
| Single-property patch p95 | <1 us (reported as 0 us) | 2,000 us | PASS |
| 100-property batch p95 | 65 us | 8,000 us | PASS |
| Transition host-sample p95 | <1 us (reported as 0 us) | 16,667 us | PASS |
| Idle repeated work | 0 | 0 | PASS |

## Scenario Coverage

Automated tests cover catalog/search/filter/cart totals, navigation and guards, all simulated
payment outcomes, opaque-reference isolation, receipt/preview generation, guest recovery,
compositor-loss shutdown, keyboard semantics, focus order, accessible state, and reduced-motion
state resolution. The signed POS and starter bundles build reproducibly, and malformed or
untrusted bundles fail before guest execution.

## Outstanding Release Evidence

The low-power `STUDIO-BENCH-1` run is intentionally deferred for this release cycle. Revisit the
hardware profile before claiming a formal performance baseline for low-cost POS deployments. This
does not block the current specification or its automated implementation evidence.

The implementation is feature-complete, but milestone publication remains blocked on evidence
which must not be fabricated:

1. A human tester must complete every native visual/audio check in
   `docs/accessibility/ACCEPTANCE.md`, record the assistive technology and scale factor, and time
   the keyboard-only catalog-to-receipt flow below two minutes.
2. Legal and release owners must confirm licenses/notices for the final linked GPUI binary and
   sign the release checklist.

No waiver was requested or granted in this report.
