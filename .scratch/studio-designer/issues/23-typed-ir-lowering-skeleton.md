# 23 [R]: Typed Studio IR and wasm lowering skeleton

**What to build:** A typed intermediate representation with AssemblyScript/wasm lowering compiles a Studio Script subset — static screen trees and navigation actions — behind the projection interface, giving hand-authored and Designer-authored projects one compiler frontend.

**Blocked by:** 22

**Status:** ready-for-agent

- [ ] An existing example application compiles through the new pipeline and exhibits identical runtime behavior
- [ ] Identical inputs produce deterministic output bytes
- [ ] Constructs outside the subset produce source-linked diagnostics rather than silent omissions
- [ ] Compiler internals stay behind the projection interface contract

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

