# Define the Studio Design model and operation algebra

Type: grilling
Status: resolved

## Question

What entities, stable identities, invariants, references, operation types, transaction semantics, and revision rules make Studio Design simultaneously editable, undoable, syncable, agent-operable, extensible, and compilable without exposing runtime implementation types as its source model?

## Answer

Studio Designer v1 has one Runtime Application per Studio Project. A project contains screens, project-owned Reusable Compositions, Studio Library references, typed interactions, and immutable revisions. Its visual source is a flat stable-ID node map with parent/child indexes; rendering and editor projections may derive nested trees from it.

Every visual node is a Primitive Node whose kind is directly admitted by the approved Studio Runtime catalog and protocol. The source model does not invent semantic renderer kinds. A supported `Badge` may be used directly; a custom `SaleBadge` is a Reusable Composition of supported primitives such as `Stack`, `Image`, `Box`, `Text`, and `Badge`. Overlay placement is represented by supported stack/layout primitives and typed placement properties; if the current protocol lacks a needed property, that is an explicit protocol/catalog enhancement, never hidden Designer-only rendering behavior.

Studio Design owns typed layout/style metadata, tokens, responsive variants, component definitions/instances, content bindings, interaction graphs, fixtures, provenance, and agent/editor context. It does not persist runtime `UiNode` objects as its source of truth. A Runtime Projection validates and transforms the source into Runtime-compatible `UiNode` trees, events, actions, and assets. The same Runtime catalog and native adapters are reused for preview and package output where possible.

All user, agent, MCP, extension, and agent-led-ingestion edits enter a typed command engine. Commands carry stable identities, preconditions, and inverse information; atomic batches produce immutable revisions and undo groups. Opaque identities remain stable across rename, move, and visual changes. The model is therefore independent of GPUI and Wasm while remaining aligned with the closed Runtime contract.

See [CONTEXT.md](../../CONTEXT.md) for the settled glossary.
