# Quickstart: Studio Script Validation

Validates the feature end-to-end. Run from the repository root with the toolchain environment
sourced (`source /tmp/opencode/studio-build-env.sh` in this session; see
`docs/development/BUILDING.md` for the checked-in procedure).

## 1. Reference component compiles to golden IR (US1)

```bash
cargo test -p studio-script --test studio_ir_v2
```

Expected: all fixtures compile with zero diagnostics; serialized IR matches
`crates/studio-script/tests/fixtures/ir-v2/*/module.json` byte-for-byte.

## 2. Unsupported constructs fail with spanned codes (US1)

```bash
cargo test -p studio-script --test studio_validate
```

Expected: every rejected-construct family produces a `STUDIO3xx` diagnostic with an exact span
and no IR.

## 3. Dev hot swap preserves state and rejects safely (US2)

```bash
cargo test -p studio-script --test studio_hot_swap
```

Expected: template-only edits preserve widget identity and state; invalid edits keep the last
valid component; incompatible type changes dispose and re-initialize; dependent modules
reevaluate in order; 10-edit scripted session completes with no partial swaps.

## 4. Differential equivalence (US3)

```bash
cargo test -p studio-script --test studio_differential
```

Expected: for every shared fixture, the Rust evaluator, the `simulate_event` oracle, and the
compiled Wasm emit identical `protocol.jsonl` sequences.

## 5. Deterministic builds and offline (US4)

```bash
cargo test -p studio-script --test studio_fingerprint
# twice; compare artifact bytes
# disconnect network for the session; repeat steps 1-4
```

Expected: byte-identical artifacts across runs; cache invalidation limited to the edited module
chain; everything passes offline.

## 6. Full gate

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Expected: all green (the only known environment-gated failure is the Sway-harness Bun test,
documented in the feature 002/003 reports).
