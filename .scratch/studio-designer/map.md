# Chart the Studio Designer authoring-to-runtime journey

Label: wayfinder:map

## Destination

Reach a complete, implementation-ready decision set for specifying Studio Designer as a first-party, offline-capable desktop authoring application with optional cloud sync, live agent editing, extensibility, and Studio Runtime-only output. The resulting decisions must be sufficient for Matt's `/to-spec` and `/to-tickets` flow to produce testable tracer-bullet implementation slices for the full authoring-to-runtime journey.

## Notes

- Domain: native visual application design, prototyping, content authoring, agent-assisted editing, secure extensions, local/cloud persistence, and Studio Runtime compilation.
- Every session should consult `wayfinder`, `domain-modeling`, and `codebase-design`; use `prototype` for UX questions and `research` for external technical facts.
- The Studio Runtime constitution remains authoritative, including host authority, closed versioned contracts, test-first evidence, retained native UI, and small traceable slices.
- Settled starting point: Studio Designer is a first-party desktop application; Studio Design is its typed source of truth; layouts are flow-first with explicit overlay/absolute placement; the Studio Library supplies assets and typed content; agent edits apply live through tracked undoable operations; local and optional cloud workspaces are required; extensions are sandboxed while native implementations remain first-party; output targets only Studio Runtime.
- Planning only: Wayfinder resolves decisions. After the map is complete and the user approves the handoff, use Matt's `/to-spec` and `/to-tickets` flow; do not create SpecKit artifacts.

## Decisions so far

- [Determine the embedded SurrealDB topology](issues/01-embedded-surrealdb-topology.md): embed RocksDB-backed SurrealDB locally behind `LocalStore`; synchronize typed Studio Design operations through a Studio Cloud API, never storage replication.
- [Audit Studio Runtime readiness for Designer](issues/02-runtime-readiness-audit.md): reuse the verified protocol, retained UI, native host, sandbox, package, and POS foundation; add Design, Library, canvas, persistence, extension, agent, and compiler modules around it.
- [Define the Studio Design model and operation algebra](issues/03-define-studio-design-model.md): use primitive Runtime catalog kinds and reusable compositions as source, with typed design metadata and a validated Runtime Projection to `UiNode` trees.
- [Fix the Studio Designer v1 capability baseline](issues/04-fix-v1-capability-baseline.md): complete every Runtime catalog kind, all Canvas device profiles, broad normalized media, full extensions, and a POS journey using host-mediated Stripe, OAuth, Surreal data, REST, and WebSockets.
- [Shape the native editor experience](issues/05-shape-native-editor-experience.md): make canvas-first Focus View the default and offer a fully supported persistent Workbench View over the same editor session and capabilities.

## Not yet specified

- Exact cloud account, service, encryption, and deployment topology after the synchronization semantics are resolved.
- Exact schema-version and migration policy after the Studio Design model is resolved.
- Exact performance, scale, crash-recovery, accessibility, and release thresholds after the editor and compilation paths are resolved.
- Exact tracer-bullet ticket split for Matt's `/to-tickets` after all architectural decisions are known.

## Out of scope

- Web and mobile Studio Designer hosts for this effort; the domain and contracts should remain portable.
- Dedicated Rust importers for HTML, design files, or other external formats; agents perform ingestion.
- Export targets other than Studio Runtime.
- A production website CMS, public publishing system, or live Designer-backed application database.
- Simultaneous multi-user editing, presence, live cursors, invitations, and organization collaboration in v1.
- Unrestricted third-party native code, raw GPUI access, HTML/CSS injection, or arbitrary rendering.
