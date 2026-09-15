# Research: Studio Toolchain and Development Workflow

**Date**: 2026-09-03 · Resolves every plan-level unknown for [spec.md](spec.md).

## R1. Where does packaging ownership move to?

**Decision**: `studio build` calls `studio_package::pack_bundle` directly. The dependency is
already declared in `crates/studio-cli/Cargo.toml` but unreferenced; `pack_bundle` already
implements the deterministic stored-ZIP bundle, RFC 8785 canonicalization, the
`studio.bundle.signature.v1` domain, and Ed25519 signing (`PackMode::Signed(seed)` /
`DevelopmentUnsigned`). `scripts/build-example.ts` becomes a delegating shim: it ensures the
`studio` binary is built (same shared-cache cargo invocation pattern as
`scripts/build-studio.ts`), spawns `studio build <example>`, and propagates the exit code.

**Rationale**: One implementation of the bundle format instead of two parallel ones (TS and Rust
currently reimplement the same spec); determinism, atomicity, and diagnostics then live in one
place. The shim keeps every documented entry (`bun run build:starter`, `build:pos`,
`build-example.ts` direct invocation) working unchanged.

**Alternatives considered**: NAPI bridge to call Rust from Bun (rejected: new build
complexity, no precedent in this workspace); keeping TS authoritative and only adding exit codes
(rejected: leaves the duplicated format and the non-atomic write in place).

## R2. Exit-code and diagnostic design

**Decision**: A small closed registry in `studio-cli` following the `studio-package`
`*ErrorCode` pattern. Exit codes: `0` success, `2` usage error, `10` validate-phase failure,
`11` compile-phase failure, `12` package-phase failure, `13` io/environment failure, `14` watch
session failure, `130` cancelled. Diagnostic codes are namespaced per phase
(`BUILD_VALIDATE_*`, `BUILD_COMPILE_ASC`, `BUILD_PACKAGE_*` with packaging reuse of
studio-package's existing `ManifestErrorCode`/`IntegrityErrorCode`/`ArchiveErrorCode` families),
emitted as the existing JSON diagnostic record shape already used by `studio check`/`fmt`
(`code`, `severity`, `message`, `line`, `column`) plus a `phase` field.

**Rationale**: Editors and CI need stable machine surfaces; the codebase already has two
precedents to copy (studio-script `STUDIO0xx` codes, studio-package error families), so no new
abstraction style is introduced. The ad-hoc `"STUDIO_IO"` literal and anyhow-string failures on
the build path are replaced.

**Alternatives considered**: anyhow-only messages (rejected: not machine-classifiable); a
full JSON-lines protocol for all output (rejected: larger surface than needed; human-readable
progress plus JSON diagnostics per problem is enough).

## R3. Determinism and atomicity

**Decision**: Bundles are written atomically (temp file in the destination directory, then
rename over the destination). Failed or interrupted builds never touch the destination. Two
determinism hazards are removed: (1) the TS packager's stale-wasm skip (`build-example.ts`
reuses `build/<name>.wasm` when present) disappears because the Rust pipeline always compiles
from source in a build request; (2) `generate_routes` and `collect_lucide` become content-stable
— they only rewrite `assembly/routes.generated.ts` and `assets/icons/*` when the rendered
content differs from what is on disk.

**Rationale**: Byte-identity (SC-002) requires identical inputs to produce identical outputs;
mtime-based caching and unconditional rewrites break both the identity guarantee and the
watcher's change detection.

**Alternatives considered**: Content-addressed wasm caching keyed by source hash (rejected for
this feature: rebuilds are already under the performance budget; caching adds invalidation risk).

## R4. Watcher design (no new dependencies)

**Decision**: Keep polling but make it content-based and correct. The `dev` loop computes a
hash of watched source files (`assembly/**`, `routes/**`, `assets/**`) **excluding** generator
outputs (`assembly/routes.generated.ts`, `assets/icons/**` lucide writes) every 500 ms, with a
300 ms settle window that coalesces rapid saves. At most one build runs at a time; a change
detected during a build marks the session dirty and exactly one trailing build runs with the
final state (supersede semantics). Ctrl-C triggers clean cancellation: the in-flight build is
stopped, no partial output exists (atomic pack already guarantees this), child runtime process
is terminated, and the process exits `130`. A `notify`-based watcher is explicitly not added.

**Rationale**: The self-trigger bug is generator churn, not polling per se; content-stable
generation plus an exclusion list removes it at the root, and hashing also fixes mtime-only
detection misses (editors that preserve mtimes). Avoiding the `notify` crate keeps the
dependency graph closed and the behavior testable deterministically.

**Alternatives considered**: inotify/`notify` crate (rejected: new dependency, less
deterministic in tests, unnecessary at three small example projects); ignoring only
`routes.generated.ts` without content-stable writes (rejected: lucide asset writes would still
churn).

## R5. Dev-mode presentation seam

**Decision**: For this feature, `studio dev` launches the runtime host binary in development
mode (`studio-app --dev <bundle>`, development trust store) through a small launch seam in the
CLI, and now surfaces spawn failures as structured diagnostics instead of swallowing them
(today the spawn result is `.ok()`-discarded). The roadmap's eventual in-process composition
(`studio` composing `studio-host`/`studio-shell`/`studio-renderer` without launching the
`studio-app` binary) is deferred to features 004/005, which introduce those crates; the launch
seam is the mechanical swap point.

**Rationale**: `studio-shell`/`studio-renderer` do not exist yet; building them is feature 004
scope. Development mode is not the production runtime path (different trust store, dev
admission), which satisfies the spec's intent of not using the production runtime path while
keeping scope honest.

**Alternatives considered**: embedding a GPUI event loop in the CLI now (rejected: duplicates
`studio-app` wholesale and blocks the toolchain tickets on UI work).

## R6. Test strategy

**Decision**: Integration tests spawn the built `studio` binary via
`CARGO_BIN_EXE_studio` (established by `crates/studio-cli/tests/replay.rs`) against fixture
projects under `crates/studio-cli/tests/fixtures/`. Coverage: clean build produces a bundle;
compile failure → exit 11 + `phase: "compile"` diagnostic + destination untouched; repeated
identical builds → byte-identical bundles; watcher harness (drives the loop logic as a library
function with injected clock/sink) for generated-churn exclusion, debounce coalescing,
single-flight supersede, and cancellation; shim parity (TS entry and native command produce
identical bundles and exit codes).

**Rationale**: Binary-level tests verify the contract users see (exit codes, diagnostics,
artifacts) rather than internals; the watcher logic is factored into a testable pure-ish module
so timing behavior is deterministic without real sleeps where possible.

**Alternatives considered**: Bun-level tests only (rejected: the behavior under test is the
native CLI contract).
