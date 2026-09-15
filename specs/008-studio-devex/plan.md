# Implementation Plan: Studio Script Developer Experience

**Branch**: `008-studio-devex` | **Date**: 2026-09-03 | **Spec**: [spec.md](spec.md)

## Summary

Extend `studio-language-server` with three composed capabilities — formatting via
`rsvelte_fmt::FormatSession`, diagnostics via `rsvelte_lint` plus a Studio rules layer,
and rename/references/definition via `rsvelte_projection` mappings — alongside
Studio-owned completion, hover, and catalog intelligence. Ship a thin VS Code client
(`editors/vscode/`) that spawns the Rust binary over stdio. No JavaScript toolchain
packages enter the closure.

## Technical Context

**Language/Version**: Rust nightly-2026-03-04; TypeScript (client only, no toolchain deps)

**Primary Dependencies**: Vendored `rsvelte_fmt`, `rsvelte_formatter`,
`tailwind_class_order`, `rsvelte_lint`, `rsvelte_diagnostics`, `rsvelte_preprocess`
(manifest-only), `rsvelte_projection` at the pinned rev; existing `studio-script`,
`studio-protocol` catalog data

**Storage**: N/A; server is stateless per request plus the in-memory Workspace index

**Testing**: cargo tests (stdio protocol, formatting goldens, diagnostic fixtures,
projection mappings, failure isolation) plus packaged-extension manual verification

**Target Platform**: Linux desktop; VS Code (any OS) for the client

**Project Type**: Language server + thin editor client

**Performance Goals**: Keystroke-latency answers from the index; upstream calls bounded
with deterministic ordering

**Constraints**: No upstream type crosses Studio-owned API boundaries unconverted; tooling
failures degrade to diagnostics; compiler/IR/build behavior identical with or without
tooling; no new npm dependencies

**Scale/Scope**: Fixture projects; single-server process per workspace

## Constitution Check

PASS. Test-first with smoke, golden, fixture, protocol, and isolation coverage. No host,
sandbox, network, or capability changes (local file reads only). Additive contracts
(server capabilities, client commands). Small slices per story.

## Project Structure

```text
specs/008-studio-devex/
├── plan.md, research.md, data-model.md, quickstart.md, contracts/, tasks.md

crates/studio-language-server/src/
├── format.rs        # FormatSession composition + idempotence (new)
├── lint.rs          # upstream diagnostics + Studio rules layer (new)
├── projection.rs    # mapping-backed rename/references/definition (new)
├── intelligence.rs  # catalog/SDK/rune/route/asset completion+hover (extend)
└── server.rs        # capability handshake + degraded modes (extend)

editors/vscode/
├── package.json     # activation, grammar, commands (new)
├── src/extension.ts # spawn server, wire commands, degrade (new)
└── syntaxes/studio.tmLanguage.json  # .studio highlighting (new)

crates/studio-language-server/tests/
├── format_golden.rs / lint_fixtures.rs / projection_maps.rs / isolation.rs (new)
```

**Structure Decision**: All server work inside `studio-language-server`; the client is a
separate `editors/` tree (not a cargo workspace member). No new crates.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| None | N/A | Fits existing boundaries. |
