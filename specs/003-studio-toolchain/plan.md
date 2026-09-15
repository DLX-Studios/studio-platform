# Implementation Plan: Studio Toolchain and Development Workflow

**Branch**: `003-studio-toolchain` | **Date**: 2026-09-03 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/003-studio-toolchain/spec.md`

## Summary

Make `studio` (crates/studio-cli) the real toolchain authority: a phased build pipeline
(validate → compile → package) with stable diagnostic codes and documented exit codes that calls
`studio_package::pack_bundle` directly (the dependency is already declared but unused), writes
bundles atomically, and produces byte-identical outputs for identical inputs. Rewrite the `dev`
watcher as a content-hash polling loop that ignores generator outputs (`routes.generated.ts`,
lucide asset writes become content-stable), debounces rapid saves, runs one build at a time with
deterministic supersede semantics, and cancels cleanly. `scripts/build-example.ts` becomes a
delegating shim that invokes the `studio` binary, keeping documented TS entries working.

## Technical Context

**Language/Version**: Rust nightly-2026-03-04 (pinned via rust-toolchain.toml), AssemblyScript
0.28.20 via asc, Bun 1.3.9 (pinned in CI and docs)

**Primary Dependencies**: clap (existing CLI), serde/serde_json, studio-package (pack_bundle,
error-code families), studio-script (existing STUDIO0xx diagnostic pattern), studio-protocol;
no new external dependencies (no `notify` crate — content-hash polling is used)

**Storage**: N/A; bundles are written to `examples/<name>/build/<name>.studio` atomically
(temp file + rename); no database involvement

**Testing**: cargo integration tests in `crates/studio-cli/tests/` (new: build pipeline, watch
session, shim parity) plus existing `scripts/test-starter-quickstart.sh` and Bun e2e
(`tests/e2e/starter_plugin.test.ts`)

**Target Platform**: Linux desktop/POS on native Wayland; toolchain itself is host-side Rust

**Project Type**: CLI toolchain inside a sandboxed native desktop runtime workspace

**Performance Goals**: Clean build of the starter example under one minute on the development
validation host; watcher reacts within one debounce window (300 ms settle) plus one build

**Constraints**: Closed protocol; no new capabilities; packaging trust semantics unchanged;
byte-identical bundles; failed builds never damage existing output; `studio-app` remains the
runtime authority

**Scale/Scope**: Three example projects (starter, pos-desktop, github-viewer); single-user local
toolchain; no remote/watchless filesystems

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

PASS. Host authority is preserved (`studio-app` keeps runtime ownership; the toolchain only
produces bundles it already admits). Test-first is honored (clean/failed/repeated build and
watcher tests precede or accompany each behavior). Contracts stay versioned (bundle format,
signature domain, and error-code families are reused, not redefined). No new capability, secret
path, network path, or storage path is introduced; packaging keeps the existing Ed25519
development-seed and trust-store semantics. Work is sliced into small traceable tasks.

## Project Structure

### Documentation (this feature)

```text
specs/003-studio-toolchain/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (CLI contract: commands, exit codes, diagnostics)
└── tasks.md             # Phase 2 output ($speckit-tasks command)
```

### Source Code (repository root)

```text
crates/studio-cli/
├── src/main.rs              # split into modules during T001
├── src/build/               # phase pipeline, diagnostics, exit codes (new)
├── src/watch.rs             # debounced single-flight watcher (new)
└── tests/
    ├── build_pipeline.rs    # clean/failed/repeated builds (new)
    ├── watch_session.rs     # churn/debounce/single-flight/cancel (new)
    └── shim_parity.rs       # TS shim == native command (new)

scripts/build-example.ts     # becomes a delegating shim (kept working)
package.json                 # build:starter/build:pos entries unchanged on the surface
examples/<name>/build/       # atomic bundle destination
```

**Structure Decision**: Extend `studio-cli` in place — split `main.rs` into `build/` + `watch`
modules, wire the already-declared `studio-package` dependency into the packaging phase, and
convert the TS packager into a shim. No new crates; no workspace-boundary changes.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| None | N/A | The toolchain fits the existing workspace boundaries; no new crates or dependencies. |
