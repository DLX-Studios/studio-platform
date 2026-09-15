# Data Model: Studio Toolchain and Development Workflow

**Date**: 2026-09-03 · Entities for [spec.md](spec.md) · implementation lives in `crates/studio-cli`.

## BuildRequest

One unit of work for the pipeline.

| Field | Type | Notes |
|-------|------|-------|
| `example` | string | Example/project name; validated against known projects |
| `project_root` | path | `examples/<example>` |
| `output_bundle` | path | `examples/<example>/build/<example>.studio` (destination, written atomically) |

Validation rules: example must exist and contain `assembly/index.ts`; output directory must be
creatable/writable (else `BUILD_PACKAGE_IO` family).

## BuildPhase (closed enum)

`Validate` → `Compile` → `Package`. Every diagnostic and every failure records its phase.

- **Validate**: project layout, manifest inputs, source presence. Failures: `BUILD_VALIDATE_*`.
- **Compile**: lucide collection, route generation (content-stable), asc compile to wasm.
  Failures: `BUILD_COMPILE_*`.
- **Package**: `studio_package::pack_bundle` + atomic write. Failures: `BUILD_PACKAGE_*` plus
  reused studio-package error families (`MANIFEST_*`, `INTEGRITY_*`, `ARCHIVE_*`).

## BuildDiagnostic

One safe, machine-readable record per problem.

| Field | Type | Notes |
|-------|------|-------|
| `phase` | BuildPhase | Owning phase |
| `code` | string | Stable, namespaced (`BUILD_COMPILE_ASC`, `ARCHIVE_LIMIT_*`, …) |
| `severity` | enum | `error` \| `warning` |
| `message` | string | Safe, actionable; never contains secret material |
| `line`/`column` | option | Present when the source is known (asc parse errors) |

Serialized as the existing JSON diagnostic record shape (`print_diagnostics` in studio-cli) plus
the `phase` field; one JSON object per problem.

## ExitCodeFamily (documented, closed)

| Code | Family |
|------|--------|
| 0 | success |
| 2 | usage error |
| 10 | validate-phase failure |
| 11 | compile-phase failure |
| 12 | package-phase failure |
| 13 | io/environment failure |
| 14 | watch session failure |
| 130 | cancelled (watch session) |

Each code uniquely identifies its family; contract in `contracts/cli.md`.

## WatchSession

Dev-loop state machine.

| Field | Type | Notes |
|-------|------|-------|
| `watched_roots` | list of paths | `assembly/`, `routes/`, `assets/` |
| `excluded_paths` | list of globs | `assembly/routes.generated.ts`, `assets/icons/**` |
| `content_hashes` | map path→hash | Replaces mtime comparison |
| `settle_window` | duration | 300 ms debounce coalescing rapid saves |
| `in_flight` | option\<BuildRequest\> | At most one build at a time |
| `dirty` | bool | A change arrived during the in-flight build → one trailing build |

State transitions: `Idle → Settling → Building → (dirty ? Settling : Idle)`; `Cancelled` from
any state on Ctrl-C (stops build, exits `130`, no partial output, no lock residue).

## Bundle

The runnable output artifact (unchanged format): deterministic stored-ZIP produced by
`studio_package::pack_bundle` with manifest, module, assets, and signature per the existing
`studio.bundle.signature.v1` domain. Byte-identical for identical inputs; replaced atomically.

## Shim Contract

`scripts/build-example.ts <example>` guarantees: ensure `studio` binary exists → spawn
`studio build <example>` → propagate exit code verbatim. Documented Bun entries
(`build:starter`, `build:pos`) keep their names and outputs.
