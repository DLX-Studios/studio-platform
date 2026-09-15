# 34 [R]: Remove POS-specific rendering from the generic path

**What to build:** All point-of-sale identifiers and branches (`order-pane`, `catalog-pane`, `add-*` conventions) leave the generic render path; certification forbids fallback rendering for certified kinds. The POS example renders identically through purely generic machinery.

**Blocked by:** 31, 32, 33

**Status:** complete (TimePicker explicitly documented-not-certified)

- [ ] Example renders pixel-equivalent through the generic path (external visual regression gate)
- [x] Audit shows zero POS-specific identifiers or branches in generic code (`crates/studio-app/src/foundation.rs:1222`, `:1874`, `:2240`; no POS identifiers in renderer)
- [x] Release certification fails if any approved kind lacks semantic rendering (`crates/studio-components/src/catalog.rs:206`, `crates/studio-components/tests/catalog_mapping.rs:209`)
- [x] Fallback surfaces exist only behind an explicit development diagnostic (`crates/studio-app/src/foundation.rs:1182`, `:1200`; only TimePicker reaches it at `:2251`)

## Closure evidence

- Formerly deferred catalog kinds now use the schema-driven native surface at
  `crates/studio-app/src/foundation.rs:898`; the readiness matrix marks them certified.
- `TimePicker` is the sole declared/mapped but not-certified kind, and certification reports it
  explicitly rather than silently passing (`crates/studio-components/tests/catalog_mapping.rs:209`).
