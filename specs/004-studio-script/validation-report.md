# Validation Report: Studio Script and Embedded Development Host

**Validation date:** 2026-09-03
**Implementation result:** T001–T027 complete. `.studio` sources compile through the pinned
`rsvelte` frontend into versioned typed IR with portable-subset validation; the Rust evaluator
projects IR onto retained trees with transactional identity-preserving hot swap; the extended
AssemblyScript backend emits standalone modules proven behaviorally identical to the evaluator;
module graphs, fingerprints, and CLI wiring close the lifecycle.

## Automated evidence

| Gate | Result | Evidence |
| --- | --- | --- |
| Formatting | PASS | `cargo fmt --all -- --check` |
| Rust lint | PASS | `cargo clippy --locked --workspace --all-targets -- -D warnings` |
| Rust workspace | PASS | `cargo test --locked --workspace` (exit 0) |
| Frontend adapter (US1) | PASS | `AdapterEngine` parses the reference set with spans and expression shapes; `EngineFingerprint` feeds cache keys; no `rsvelte` type crosses `studio-script` (T003) |
| Portable validation (US1) | PASS | `cargo test -p studio-script --test studio_validate` — every rejected family yields its `STUDIO3xx` code with exact spans; six reference fixtures validate clean |
| Golden IR (US1) | PASS | `cargo test -p studio-script --test studio_ir_v2` — byte-identical `module.json` goldens with positional/author-stable IDs; repeated compilations identical |
| Rust evaluator (US2) | PASS | `cargo test -p studio-script --test studio_eval` — mount projections with unique IDs, typed dispatches, derived evaluation, keyed rows, required-prop failures |
| Hot swap (US2) | PASS | `cargo test -p studio-script --test studio_hot_swap` — scripted session: preserve/dispose/initialize, rejection-and-continue, removal semantics, planning purity |
| ASC backend (US3) | PASS | `cargo test -p studio-script --test studio_emit` — deterministic standalone modules with handler branches and valid mount payloads; multi-component correctly scoped to the graph |
| Differential (US3) | PASS | `cargo test -p studio-script --test studio_differential` — evaluator and compiled-Wasm sequences byte-identical for all shared fixtures; `protocol.jsonl` goldens reviewed |
| Graph + fingerprints (US4) | PASS | `studio_graph` (resolution, named cycles, ordered invalidation, virtual contracts) and `studio_fingerprint` (identical inputs key identically; any drift rekeys) suites |
| CLI wiring (US4) | PASS | `studio check` validates `.studio` with `STUDIO3xx`; `studio build` compiles studio-entry projects end to end; watcher covers `components/` and top-level `.studio` |
| JavaScript gates | PASS | `bun run check` exit 0; root `bun test` 38 pass with only the environment-gated Sway-harness test (host runs KDE Wayland; compositor evidence in the 001 report) |

## Scope and limitations

The single canonical `number` decision (language level) with deterministic Integer/Decimal
host mapping is enforced by the validator and covered by unit tests. Payloads are mount-time
constants because v2 has no state-mutation operations; when such operations arrive, the ASC
backend must become dynamic (noted in `emit_studio`). Multi-component modules need the module
graph composer; multi-entry projects are out of scope. Live in-session HMR display belongs to
feature 005 (reload protocol); `studio dev` rebuilds on `.studio` changes through the existing
loop. The vendored `rsvelte` crates carry manifest-only lint deltas (see
`vendor/rsvelte/UPSTREAM.md`); sources are byte-identical to the pinned rev. Emitted `wasm`
artifacts differ across ASC environments by toolchain drift, not by packaging; format
equivalence was proven in feature 003.
