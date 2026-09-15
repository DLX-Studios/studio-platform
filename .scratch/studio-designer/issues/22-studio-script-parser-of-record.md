# 22 [R]: Studio Script parser of record, canonical printer, check/fmt

**What to build:** The rsvelte-flavored grammar parses `.studio` sources into a stable-ID semantic model; a deterministic canonical printer serializes it back; `studio check` validates and reports structured diagnostics; `studio fmt` enforces canonical formatting. Comments persist as trivia anchored to their following node.

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] Round-trip invariant holds: parsing printed output reproduces the identical model
- [x] Printing is deterministic: identical models yield byte-identical output across runs and machines
- [x] Missing, duplicate, or non-canonical node identities are rejected with line-linked diagnostics
- [x] Comments survive parse/print cycles anchored to their nodes
- [x] Check exits nonzero with stable diagnostic codes on the invalid-fixture corpus
- [x] Fixture corpus covers valid and invalid documents including hostile input

## Implementation notes

- Added a bounded, closed Studio Script v1 parser with a documented `studio 1`
  header, `lang="studio"` script block, nested catalog/plugin elements,
  canonical lower-kebab stable IDs, bounded `$item.*` bindings, token refs,
  text, and anchored comment trivia.
- Added a deterministic semantic model and canonical printer with
  `parse(print(model)) == model` and idempotent formatting tests.
- Added stable line/column diagnostics for syntax, version, ID, expression,
  script, and hostile-input-limit failures, plus valid/invalid fixture corpus.
- Added `studio check` and `studio fmt [--check]`; both emit structured JSON
  diagnostics and never pull GPUI or SurrealDB into the parser boundary.
