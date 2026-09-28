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
- [Define Studio Library ownership and bindings](issues/06-define-studio-library-model.md): project-owned Library with content-addressed `asset-sha256-{hash}` identity, provenance merge on deduplication, typed bindings failing as `CONTENT_BUILD_BLOCKED`, reference-aware `RequireUnbound` deletion, and immutable packaged snapshots.
- [Define full extension authority](issues/07-define-extension-authority.md): sandboxed declarative extensions behind signed descriptors that are rejected before activation when tampered or incompatible; capabilities are consent-recorded and revocable, lifecycle hooks are bounded and contained, and native renderer implementations stay first-party.
- [Define live agent and MCP editing semantics](issues/08-define-live-agent-editing.md): live typed command batches through the validated engine with no proposal gate and `studio check` self-correction; overlapping stale batches resolve last-writer-wins at batch granularity and each accepted batch undoes independently with no cross-batch task grouping.
- [Define offline and cloud synchronization](issues/09-define-sync-conflict-model.md): offline-first with optional same-user cloud sync through an idempotent operation outbox and cursor pulls, content-addressed asset transfer, periodic logical snapshots, and explicit recoverable conflicts that never silently overwrite.
- [Define preview and Runtime compilation](issues/10-define-preview-compiler-pipeline.md): one validated Runtime Projection feeds both identity-preserving native previews and deterministic signed Runtime packages, with manifest asset lists matching packaged keys exactly.
- [Define Designer trust and capability boundaries](issues/11-define-trust-and-capabilities.md): separated trust domains with capability checks at every closed-protocol boundary; guests, agents, MCP clients, and extensions never receive a database handle or query language, and compiler output is signed and verified before launch.
- [Set quality and release gates](issues/12-set-quality-and-release-gates.md): measurable functional, performance, accessibility, recovery, security, synchronization, determinism, and end-to-end gates with recorded evidence; gaps become explicit waivers with named owners, never silent skips.
- [Shape the specification and tracer-bullet handoff](issues/13-decompose-matt-delivery.md): synthesize through `/to-spec`, then decompose through `/to-tickets` into dependency-ordered, test-first tracer bullets landing through format, workspace-test, and Clippy integration checkpoints.

## Not yet specified

- Exact cloud account, service, encryption, and deployment topology; synchronization semantics are settled in [09](issues/09-define-sync-conflict-model.md).
- Exact performance budgets and baseline hardware for the benchmark set, recorded before release; the gate set itself is fixed in [12](issues/12-set-quality-and-release-gates.md).

## Out of scope

- Web and mobile Studio Designer hosts for this effort; the domain and contracts should remain portable.
- Dedicated Rust importers for HTML, design files, or other external formats; agents perform ingestion.
- Export targets other than Studio Runtime.
- A production website CMS, public publishing system, or live Designer-backed application database.
- Simultaneous multi-user editing, presence, live cursors, invitations, and organization collaboration in v1.
- Unrestricted third-party native code, raw GPUI access, HTML/CSS injection, or arbitrary rendering.
