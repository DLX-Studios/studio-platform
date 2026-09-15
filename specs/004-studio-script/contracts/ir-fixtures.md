# Contract: Studio IR Golden Fixtures

**Status**: Contract for feature 004. Fixtures are versioned by `STUDIO_IR_VERSION` (= 2);
any IR change bumps the version and regenerates goldens in the same commit.

## Fixture layout

`crates/studio-script/tests/fixtures/ir-v2/<name>/` contains:

- `source.studio` — the exact author-facing input.
- `module.json` — the expected `StudioModule` serialized canonically (stable IDs, sorted
  maps, spans as `{start,end}` byte offsets).
- `protocol.jsonl` — one JSON Studio protocol message per line: the expected observable
  output when the module mounts and the fixture event sequence replays.

Reference fixture set: `props-defaults`, `state-derived`, `conditional`, `keyed-each`,
`interpolation`, `typed-events`.

## Equivalence rule

For every fixture, the Rust evaluator, the `simulate_event` oracle, and the compiled-Wasm
artifact must emit `protocol.jsonl` byte-for-byte, in order, given the same event sequence.
Any divergence fails the differential suite; fixture updates require the updated protocol
output to be reviewed as evidence, not just re-recorded.
