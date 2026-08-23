# Studio Runtime readiness audit for Designer

Audit snapshot: 2026-08-23. This is a source audit, not a fresh release certification.

## Ready and reusable

- `studio-protocol` has a closed protocol-v1 contract with `MountTree`, `UiNode`, a broad `NodeKind` catalog, `PatchBatch`, navigation/actions/events, deny-unknown-fields decoding, and bounded message/tree/patch limits. See `crates/studio-protocol/src/lib.rs`, `src/ui.rs`, and `src/properties.rs`.
- `studio-ui` provides stable owner-scoped node identity, retained trees, atomic mount/patch transactions, structural validation, parent links, and patch metrics. See `crates/studio-ui/src/registry.rs` and `src/transaction.rs`.
- `studio-components` provides protocol-to-native mapping, focus/accessibility metadata, retained native state, input dispatch, secret-input handling, reduced-motion policy, and targeted update tests. It explicitly rejects HTML/CSS/native-class/raw-drawing/shader/device-control properties.
- `studio-app` has a real Wayland-only GPUI host, bundle verification/instantiation, one mounted plugin surface, a native render pass, host-owned route/confirmation surfaces, and an integrated POS reference journey.
- `studio-package` produces deterministic signed `.studio` archives containing `manifest.json`, `module.wasm`, `signature.ed25519`, and declared assets. `studio-wasm`/`studio-security` enforce Wasmtime limits, no WASI, principals, and host-owned capabilities.
- The POS example demonstrates a real guest application with native layout, image assets, text, buttons, inputs, select, slider, patches, local state, and a catalog/cart/checkout/receipt/print-preview journey.

## Partial or not ready for Designer

- The native render pass is a first-party foundation implementation, not a generic design renderer. Many declared node kinds use generic fallback surfaces or hard-coded demonstrations; several components are mapped for validation but lack semantically complete rendering and editing behavior. The render code also contains POS-specific ID conventions such as `add-*`, `*-img`, `order-pane`, and `catalog-pane`.
- The current protocol tree is a runtime projection, not a source model. It has no design tokens, semantic style layers, responsive variants, overlay placement metadata, component definitions/instances, content bindings, interaction graph, fixtures, provenance, or source-level migrations.
- There is no Studio Designer canvas: no viewport/frame model, hit testing, selection overlays, hierarchy editing, inspector, drag/drop layout commands, absolute overlay editing, visual history, or library surface.
- There is no Studio Library persistence or runtime snapshot contract. Bundles admit declared byte assets, but the manifest has only the current payment/printer simulator capabilities and no content collections, asset metadata/provenance, library bindings, or host storage service.
- `studio-script` currently only splits a source file into a script block and markup. The `rsvelte` adapter, typed Studio IR, Rust evaluator, hot swap, and AssemblyScript lowering remain planned in `docs/STUDIO_SCRIPT_TRANSFORM_PIPELINE.md`.
- `studio-cli` can build the checked-in AssemblyScript examples, generate routes, collect Lucide assets, pack, and launch a preview. Its dev mode is an example watcher with polling and an HMR placeholder; it does not compile a Studio Design document.
- The host currently mounts one selected plugin surface. Runtime protocol support for multi-screen composition and broader storage/network capabilities is not a Designer persistence or application-data model.
- Runtime release evidence still has outstanding human accessibility/legal sign-off and low-power hardware certification. The component-platform roadmap says complete, while `specs/002-component-platform/spec.md` remains marked Draft; this source-of-truth inconsistency must be resolved in the Designer plan.

## Baseline consequence

Studio Designer can reuse the protocol, retained registry, native state/event modules, host security, package format, and reference application. It needs new first-party modules for the typed Studio Design source model, command/revision engine, Studio Library, native design canvas, generic component render adapters, preview/compiler bridge, local SurrealDB store, cloud operation sync, live agent/MCP commands, and extension registration.

The first Designer component matrix must distinguish `protocol declared`, `native mapped`, `native rendered`, `editable`, `runtime verified`, and `release certified`; a `NodeKind` enum entry alone is not evidence of Designer readiness.
