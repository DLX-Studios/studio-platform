# Contract: `.studio` CLI Surface (additive to the feature 003 contract)

**Status**: Contract for feature 004. Additive to
`specs/003-studio-toolchain/contracts/cli.md`; the existing exit-code families and diagnostic
shape apply unchanged.

## `studio check` accepts `.studio` sources

`studio check` validates `.studio` files against the portable subset. Rejections use the
`STUDIO3xx` families with exact spans and exit in the validate family (10). Valid files emit
the existing ok-status JSON.

## `studio build` compiles `.studio` projects

For projects whose entry is a `.studio` module, the compile phase lowers source → Studio IR →
AssemblyScript (content-stable generated module) → ASC → Wasm, then packages unchanged. The
compile-phase diagnostics carry the `.studio` span; failures exit 11 with no partial bundle.

## `studio dev` hot-swaps Studio IR

Source saves trigger parse → validate → lower → atomic swap with the swap contract
(preserve-by-stable-ID-and-type, reject-and-keep-last-valid, rollback on host failure).
Swap outcomes are diagnostics, not failures: rejected swaps do not exit the session.

## Offline and determinism

Parse, validate, swap, and ASC compilation run offline after install. Identical sources and
pinned toolchain versions produce byte-identical IR, AssemblyScript, and Wasm.
