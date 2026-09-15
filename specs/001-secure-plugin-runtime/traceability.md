# Milestone-One Requirement Traceability

Each requirement appears exactly once. “Automated” means the cited command is a release gate;
manual checks may supplement it but are never silently treated as automated evidence.

| Requirement | Coverage | Evidence type | Evidence |
| --- | --- | --- | --- |
| FR-001 | Production integrity trust identity capability compatibility and resource verification | Automated | `signature_verification`, `manifest_v1`, and `plugin_launch::valid_signed_bundle_mounts_before_exposing_the_surface` |
| FR-002 | Bounded deterministic archive and closed manifest | Automated | `cargo test -p studio-package --tests` |
| FR-003 | Explicit selected unsigned developer mode with persistent warning and retained controls | Automated | `plugin_launch::unsigned_development_launch_requires_explicit_dev_and_keeps_warning` |
| FR-004 | One import ABI with no WASI or ambient authority | Automated | `cargo test -p studio-wasm --test module_policy` |
| FR-005 | Closed bounded protocol and lifecycle messages | Automated | `protocol_v1` and `developer_errors` |
| FR-006 | Fuel epoch memory table and queue containment | Automated | `runtime_limits` plus fuzz targets |
| FR-007 | Terminal lifecycle and host-owned recovery | Automated | `plugin_recovery` |
| FR-008 | Complete closed native component catalog | Automated | `component-catalog.test.ts` and `catalog_mapping` |
| FR-009 | Complete initial hierarchy validation before any visible commit | Automated | `mount_registry` |
| FR-010 | Stable-ID property insert remove and replace operations | Automated | `patch_transaction` |
| FR-011 | Whole-batch validation and atomic commit or rejection | Automated | `patch_transaction` |
| FR-012 | Unaffected identity focus scroll and input retained | Automated | `targeted_updates` and `catalog_cart` |
| FR-013 | Owner-checked typed UI event dispatch | Automated | `catalog_mapping::dispatches_typed_non_secret_events` |
| FR-014 | Fine-grained reactive bindings and batched patches | Automated | `reactivity.test.ts` and `widget-bindings.test.ts` |
| FR-015 | Deterministic derived values disposal and bounded cycle failure | Automated | `reactivity.test.ts` |
| FR-016 | Nested parameterized lazy routes plus complete bounded stack command set | Automated | `route_tree` and `navigation_stack` |
| FR-017 | Host validation guards ownership and timeout before stack mutation | Automated | `navigation_stack` |
| FR-018 | Host-clock transitions and reduced motion | Automated | `transitions` and `accessibility` |
| FR-019 | Host-owned sensitive inputs and opaque references | Automated | `secret_input` and `protected_payment` |
| FR-020 | Raw secrets never enter guest messages or snapshots | Automated | `redaction` and `secret_input` |
| FR-021 | Exact principal purpose and session scoping | Automated | `opaque_handles` |
| FR-022 | Trusted merchant amount currency and simulator confirmation | Automated | `payment_confirmation` and `protected_payment` |
| FR-023 | Trusted immutable payment confirmation | Automated | `payment_confirmation` and `protected_payment` |
| FR-024 | Four deterministic offline payment outcomes | Automated | `payment_simulator` and `pos_checkout` |
| FR-025 | Bounded no-eviction process idempotency | Automated | `payment_simulator::capacity_never_evicts_terminal_records` |
| FR-026 | Exact integer money with explicit currency | Automated | `payment_confirmation` and `receipts` |
| FR-027 | Approved immutable structured receipts | Automated | `receipts` and `checkout_navigation` |
| FR-028 | Structured-only one-job printer preview | Automated | `printer_simulator` |
| FR-029 | Secrets and live references absent from artifacts | Automated | `cargo test -p studio-app --test redaction` |
| FR-030 | Keyboard focus labels states and target semantics | Automated | `cargo test -p studio-app --test accessibility` |
| FR-031 | Native Wayland only and terminal compositor loss | Automated | three no-X11 scripts and `compositor_disconnect` |
| FR-032 | Coherent reference POS vertical slice | Automated | `catalog_cart` and `pos_checkout` |
| SC-001 | Warm usable screen under 150 ms | Automated | `STUDIO-BENCH-1 warm_first_frame_p50_us` |
| SC-002 | Ordinary interaction p95 under 100 ms | Automated | `STUDIO-BENCH-1 interaction_p95_us` |
| SC-003 | Catalog-to-receipt task under two minutes | Manual | `pos_checkout` verifies the flow; `docs/accessibility/ACCEPTANCE.md` requires the timed native operator check |
| SC-004 | Every payment outcome has documented recovery | Automated | `pos_checkout::every_terminal_payment_outcome` |
| SC-005 | Hostile inputs contained with responsive shell | Automated | hostile Rust suites fuzz smoke and `plugin_recovery` |
| SC-006 | Secret-isolation suite finds no raw value leakage | Automated | `redaction` `opaque_handles` and `protected_payment` |
| SC-007 | Repeated idempotency identity yields one result | Automated | `payment_simulator::retained_keys_replay_exactly` |
| SC-008 | Checkout fully keyboard operable and labeled | Manual | `accessibility` plus the native checks in `docs/accessibility/ACCEPTANCE.md` |
| SC-009 | Distributed executable has no X11 XCB or XWayland | Automated | release linkage feature and headless scripts |
| SC-010 | Idle plugin emits no continuous work | Automated | `idle_plugin` and `STUDIO-BENCH-1 idle_repeated_work` |
| SC-011 | Starter build package launch diagnose under ten minutes | Automated | `test-starter-quickstart.sh`; latest result 2 seconds |
| SC-012 | Complete evidence matrix before release | Automated | `bun test tests/traceability.test.ts` |
