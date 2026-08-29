# 23 [R]: Typed Studio IR and wasm lowering skeleton

**What to build:** A typed intermediate representation with AssemblyScript/wasm lowering compiles a Studio Script subset — static screen trees and navigation actions — behind the projection interface, giving hand-authored and Designer-authored projects one compiler frontend.

**Blocked by:** 22

**Status:** closed for the verified subset; one external example-input gap is recorded below

- [ ] An existing example application compiles through the new pipeline and exhibits identical runtime behavior — **external gap, not claimed:** `examples/starter` and `examples/pos-desktop` contain hand-authored AssemblyScript entrypoints, while `examples/pos-desktop/build/pos-desktop.studio` is a binary ZIP bundle and neither example has a Studio Script source document to feed to `studio_script::compile`. The checked-in lowering fixture provides the narrowest reproducible source → IR → emitted behavior parity (`crates/studio-script/tests/wasm_emission.rs:73-117`) but is not mislabeled as an existing example. Migrating an example to Studio Script is a follow-up input/content task.
- [x] Identical inputs produce deterministic output bytes — `crates/studio-script/tests/wasm_emission.rs:37-51` asserts repeated emission and semantically equivalent source formatting are byte-identical.
- [x] Constructs outside the subset produce source-linked diagnostics rather than silent omissions — `crates/studio-script/tests/lowering.rs:99-126` covers binding, token, and unknown-kind rejection; `crates/studio-script/tests/lowering.rs:129-171` covers source-linked behavior diagnostics.
- [x] Compiler internals stay behind the projection interface contract — `crates/studio-script/src/lib.rs:62-91` exposes `compile` as the source-to-IR seam while parser/lowering/backend types remain crate-owned.

## Implementation notes

- Added `crates/studio-script/src/ir.rs`: closed, versioned (`STUDIO_IR_VERSION = 1`)
  IR mirroring the parser semantic model — screens with derived `/<id>` routes,
  element/text trees, static literal properties, and declarative navigation
  actions mirroring the v1 protocol `NavigationCommand` set.
- Added `src/lower.rs`: document → IR lowering plus a closed line-oriented
  behavior grammar consumed from the `<script lang="studio">` block
  (`on <pressed|changed|submitted> <node-id> push|replace|pop-to|reset(<route>)
  | pop()`). Stable diagnostic codes `STUDIO201`–`STUDIO208`; nothing outside
  the subset is silently omitted.
- Added `src/assemblyscript.rs`: deterministic IR → AssemblyScript emitter in
  the exact conventions of `examples/starter`/`examples/pos-desktop`
  (`studio_init` mount payload, `studio_event` navigation dispatch), plus a
  pure-Rust `simulate_event` oracle so observable behavior is testable without
  invoking ASC. Wasm compilation deliberately reuses the existing example
  build machinery (`asc` + `asconfig.json`); no parallel toolchain.
- Determinism: `emit` is a pure function of the IR; identical semantics under
  different source formatting produce byte-identical output. No
  non-deterministic seams inside the compiler; determinism ends at the ASC
  boundary owned by example/package build config.
- Lowering preserves authored child order (observable in a static tree) and
  normalizes property keys before emission; `crates/studio-script/tests/lowering.rs:79-96`
  guards the child-order invariant.
- Parity fixture: `fixtures/lowering/nav-app.studio` +
  `fixtures/lowering/nav-app.handwritten.ts`; the compiled mount payload and
  all three navigation responses are asserted byte-identical to the reviewed
  hand-written literals.
- UNVERIFIED items (API/toolchain uncertainties to confirm when cargo access
  is available):
  - Element/source spans are not preserved by the parser-of-record model;
    lowering re-locates them by scanning `id="<node-id>"` in the raw source.
    Quoted attribute values containing `>` can shift `<script>` region
    relocation (reported explicitly, never skipped).
  - Control-character escapes in generated AssemblyScript string literals use
    `\uXXXX`; confirm the pinned AssemblyScript revision accepts that form.
  - `Cargo.lock` was hand-edited to add `serde_json`/`studio-protocol` edges to
    `studio-script`; verify `cargo test --locked` accepts it without update.
  - Token references (`token.*`) are rejected (`STUDIO207`) until the Library
    snapshot joins projection; binding paths (`$item.*`) are rejected
    (`STUDIO201`) as dynamic constructs.

## Closure audit (2026-08-29)

`cargo test --locked -p studio-script` passes. The AssemblyScript
backend is deterministic Rust emission; actual ASC/Wasm compilation remains
owned by each example's existing build machinery and is not asserted by this
crate. The external example-input gap above is intentionally explicit.
