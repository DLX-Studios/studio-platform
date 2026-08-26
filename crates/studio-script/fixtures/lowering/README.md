# Lowering fixture corpus

Shared by the typed-IR lowering tests (`tests/lowering.rs`,
`tests/wasm_emission.rs`).

- `nav-app.studio` is the canonical two-screen sample: static screen trees plus
  declarative navigation behaviors in the `<script lang="studio">` block.
- `nav-app.handwritten.ts` is the reviewed hand-written AssemblyScript
  counterpart following the `examples/starter` guest conventions.  Its string
  literals are the observable-action contract the compiled module must match
  byte-for-byte.
