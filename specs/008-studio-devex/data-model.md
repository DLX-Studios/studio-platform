# Data Model: Studio Script Developer Experience

**Date**: 2026-09-03 · Entities for [spec.md](spec.md).

## FormatRequest / FormatResponse

Source plus filepath in; formatted source or a diagnostic out. Idempotent by contract.

## DiagnosticStream

One ordered list merging upstream diagnostics (codes verbatim) and Studio diagnostics
(`STUDIO3xx`), each with code, message, span.

## ProjectionMapping

Source↔structure offset maps with gap reporting; backs definition, rename, references.

## CompletionCatalog

Closed items (catalog components with props/events, SDK helpers, runes, routes, assets)
with documentation strings.

## ClientSession

Extension lifecycle states: `starting`, `ready`, `degraded` (server unavailable),
`stopped`. Commands available in `ready`; highlighting plus local commands in `degraded`.
