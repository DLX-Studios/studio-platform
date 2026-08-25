# 23 [R]: Typed Studio IR and wasm lowering skeleton

**What to build:** A typed intermediate representation with AssemblyScript/wasm lowering compiles a Studio Script subset — static screen trees and navigation actions — behind the projection interface, giving hand-authored and Designer-authored projects one compiler frontend.

**Blocked by:** 22

**Status:** ready-for-agent

- [ ] An existing example application compiles through the new pipeline and exhibits identical runtime behavior
- [ ] Identical inputs produce deterministic output bytes
- [ ] Constructs outside the subset produce source-linked diagnostics rather than silent omissions
- [ ] Compiler internals stay behind the projection interface contract
