# Audit Studio Runtime readiness for Designer

Type: task
Status: resolved

## Question

Which Studio Runtime components, properties, actions, asset paths, compiler stages, package features, and host capabilities are implemented and verified today, and which gaps must Studio Designer either add or explicitly defer to complete the authoring-to-runtime journey?

## Answer

The reusable foundation is substantial: closed protocol-v1 `UiNode`/`PatchBatch` contracts and limits; stable retained `studio-ui` trees with atomic patches; native component mapping, state, input dispatch, accessibility, reduced motion, and secret handling; a Wayland-only GPUI host; Wasmtime/no-WASI isolation; signed deterministic `.studio` packages; and the POS catalog → cart → checkout → receipt → print-preview reference flow.

The runtime is not yet a general Designer renderer/compiler. The render pass is a POS-oriented foundation with many declared kinds falling back to generic surfaces or hard-coded behavior. The protocol tree has no design tokens, responsive/overlay metadata, component definitions, content bindings, interaction graph, fixtures, or source migrations. There is no visual canvas, hit testing, selection/inspector, Studio Library, library snapshot contract, local persistence, cloud sync, extension registry, or generic design-to-runtime compiler. `studio-script` only splits source blocks; typed IR, Rust evaluation, hot swap, and AssemblyScript lowering are planned. The CLI builds checked-in AssemblyScript examples, not Studio Design documents.

Therefore Designer should reuse the existing runtime contracts and host modules through deep adapters, while adding first-party Design modules for source state, generic rendering/editing, Library, persistence/sync, agents/MCP, extensions, and compilation. The first component matrix must separately record protocol declaration, native mapping, semantic rendering, editability, runtime verification, and release certification. The roadmap/spec status mismatch (`002` roadmap marked complete while its spec is Draft) must be reconciled in the delivery plan.

Detailed evidence is recorded in [runtime-readiness-audit.md](../research/runtime-readiness-audit.md).
