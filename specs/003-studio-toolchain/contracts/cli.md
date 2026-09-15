# CLI Contract: `studio` Toolchain

**Status**: Contract for feature 003. Additive changes only after this document lands; existing
documented entries (`build:starter`, `build:pos`, `build-example.ts`) must keep working.

## Commands

### `studio build <example>`

Builds one example project into a runnable bundle.

- Phases, in order: `validate` → `compile` → `package`.
- On success: writes `examples/<example>/build/<example>.studio` atomically; exits `0`.
- On failure: prints one JSON diagnostic per problem (see shape below), exits with the phase's
  family code, and leaves the destination untouched (no partial bundle, previous bundle intact).

### `studio dev <example>`

Builds once, presents the surface through the runtime host in development mode, then watches
sources.

- Excludes generator outputs from change detection (`assembly/routes.generated.ts`,
  `assets/icons/**`).
- Debounces rapid saves (300 ms settle); at most one build at a time; a change arriving during a
  build yields exactly one trailing rebuild with the final state.
- Ctrl-C: stops the in-flight build and the runtime process, leaves no partial output, exits
  `130`.
- Runtime spawn failures are structured diagnostics (`BUILD_WATCH_RUNTIME`), never silent.

### `studio preview <example>` / `studio check` / `studio fmt` / `studio replay`

Unchanged behavior, except that all failures now use the same diagnostic record shape and
documented exit codes below. `preview` failures use the watch family (14); `check` and `fmt`
failures use the validate family (10) with io/environment problems in family 13; `replay`
failures use family 13.

## Exit codes

| Code | Family |
|------|--------|
| 0 | success |
| 2 | usage error (unknown command/argument) |
| 10 | validate phase failure |
| 11 | compile phase failure |
| 12 | package phase failure |
| 13 | io/environment failure |
| 14 | watch session failure |
| 130 | cancelled |

Each failure family maps to exactly one code; automation may classify outcomes from the code
alone.

## Diagnostic record

One JSON object per problem, on stderr:

```json
{
  "phase": "compile",
  "code": "BUILD_COMPILE_ASC",
  "severity": "error",
  "message": "asc: assembly/index.ts:12:3 - ...",
  "line": 12,
  "column": 3
}
```

- `phase`: `validate` | `compile` | `package` | `watch` | `usage` | `io`
- `code`: stable, namespaced. Packaging reuses studio-package families
  (`MANIFEST_*`, `INTEGRITY_*`, `ARCHIVE_*`) verbatim.
- `severity`: `error` | `warning`
- `line`/`column`: present when a source location is known; otherwise omitted.

## Shim contract

`bun run ./scripts/build-example.ts <example>`:

1. Ensures the `studio` binary is built (shared-cache cargo invocation).
2. Spawns `studio build <example>`.
3. Propagates the child exit code verbatim and produces the identical bundle.

## Guarantees

- **Determinism**: identical inputs → byte-identical bundles (repeated-build property).
- **Atomicity**: the destination bundle is replaced by rename; failures and cancellations never
  leave partial output and never damage an existing good bundle.
- **Stability**: diagnostic codes and exit codes are additive-only; existing codes never change
  meaning.
