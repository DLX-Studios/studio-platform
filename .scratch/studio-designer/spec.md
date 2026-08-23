# Studio Designer authoring-to-Runtime specification

Status: ready-for-agent

## Problem Statement

Studio Platform has a secure native Runtime, a closed protocol, retained native UI infrastructure, signed application packages, and a POS reference application, but it does not yet provide a general first-party visual authoring environment. Studio Canvas explored some of that role, while Instatic demonstrates an effective node-oriented desktop editor experience, but neither supplies the required Rust-native, offline-capable, Runtime-aligned authoring journey.

Users need one desktop application in which they can visually design, prototype, populate, validate, and build a Studio Runtime Application without treating HTML, `UiNode`, or a generated runtime tree as editable source. The application must preserve native Runtime semantics, support the complete approved component catalog, provide safe live agent editing, work offline with optional cloud synchronization, and prove the result by launching a signed application in the real Studio Runtime.

The existing Runtime also lacks several capabilities required by that journey. Its generic native rendering coverage is incomplete, its current render path contains POS-specific behavior, and it does not yet provide the Studio Design projection, Studio Library snapshot, embedded application data, Stripe, OAuth, REST, or WebSocket host capabilities required by the target reference application. Those Runtime additions are prerequisites within this specification, even when they are implemented before the Designer itself.

## Solution

Build Studio Designer as a first-party Rust and GPUI desktop application inside Studio Platform. Each Studio Project owns one typed Studio Design and produces one Studio Runtime Application. Studio Design is a stable-ID, command-editable source model made from approved Primitive Nodes, project-owned Reusable Compositions, responsive layout and style metadata, design tokens, typed interactions, and Studio Library bindings.

All human, agent, MCP, ingestion, and extension changes pass through one validated `DesignerSession` interface. Atomic command batches create immutable revisions and undo groups, update the host-owned embedded SurrealDB store, and optionally enter a typed cloud synchronization outbox. The Designer provides a canvas-first Focus View by default and a persistent Workbench View for deep inspection; both are presentations of the same session and expose the same capabilities.

A Runtime Projection validates Studio Design and deterministically produces Runtime-compatible native trees, events, actions, Library snapshots, and signed `.studio` packages. Preview and packaged execution reuse the approved Runtime catalog and native adapters. The POS reference journey proves the complete path with real embedded data and official sandbox integrations mediated exclusively by the Runtime host.

## User Stories

1. As a designer, I want to create a Studio Project locally, so that I can begin designing without an account or network connection.
2. As a designer, I want one Studio Project to represent one Runtime Application, so that project and application ownership remain unambiguous.
3. As a designer, I want Studio Design to remain the editable source of truth, so that generated runtime structures never replace my authoring data.
4. As a designer, I want to add every approved Runtime component kind, so that visual authoring does not expose a partial platform.
5. As a designer, I want a generic Box primitive, so that I can create ordinary styled containers without inventing unsupported semantic nodes.
6. As a designer, I want unknown component kinds to fail with useful diagnostics, so that a silent generic fallback cannot hide incompatibility.
7. As a designer, I want to combine primitives into Reusable Compositions, so that I can build project-specific components without adding native renderer kinds.
8. As a designer, I want composition instances to retain identity and update from their definition, so that reuse remains predictable.
9. As a designer, I want to detach or override only properties admitted by the composition contract, so that reuse does not become hidden duplication.
10. As a designer, I want flow-first row, column, grid, stack, and container layout, so that responsive interfaces are easy to construct.
11. As a designer, I want explicit overlay and absolute placement inside supported layout primitives, so that badges, chips, and controls can sit over media without special semantic components.
12. As a designer, I want snapping, guides, alignment, distribution, sizing, padding, gaps, and constraints, so that precise layouts can be authored visually.
13. As a designer, I want drag, resize, reorder, reparent, duplicate, group, and delete operations to be reversible, so that direct manipulation is safe.
14. As a designer, I want stable selection through rename, move, responsive changes, and view switching, so that the editor does not lose my context.
15. As a designer, I want a hierarchy view backed by stable node identities, so that I can understand and reorganize complex designs.
16. As a designer, I want multiple screens and typed navigation between them, so that I can prototype a complete application journey.
17. As a designer, I want a canvas-first Focus View, so that composition receives most of the available screen space.
18. As a designer, I want a persistent Workbench View, so that hierarchy, Library, inspector, diagnostics, interactions, agents, and history can be visible together.
19. As a designer, I want to switch between Focus and Workbench views without losing state, so that workspace preference never changes the design.
20. As a designer, I want Focus View to expose every Workbench capability through panels or commands, so that compactness does not reduce functionality.
21. As a designer, I want each view to remember its own panel arrangement, so that switching views preserves how I work.
22. As a designer, I want a command bar that can find primitives, assets, content, screens, actions, and editor commands, so that advanced workflows remain fast.
23. As a designer, I want keyboard shortcuts for common editor operations, so that the desktop application supports efficient professional use.
24. As a designer, I want canvas zoom, pan, fit, rulers, and frame controls, so that large projects remain navigable.
25. As a designer, I want to preview phone, foldable, tablet, laptop, desktop, ultrawide, television, and 4K profiles, so that one design can cover the full Canvas profile matrix.
26. As a designer, I want to edit base properties and breakpoint overrides, so that responsive behavior remains explicit and inspectable.
27. As a designer, I want profile metadata for orientation, safe areas, pixel ratio, input modes, reduced motion, and remote focus, so that previews reflect more than viewport width.
28. As a designer, I want to compare responsive profiles, so that unintended differences are visible before building.
29. As a designer, I want design tokens for color, typography, spacing, radius, border, shadow, and motion, so that visual decisions remain consistent.
30. As a designer, I want token use and overrides shown in the inspector, so that I can distinguish shared intent from local values.
31. As a designer, I want component-specific inspectors generated from approved property schemas, so that every editable property is valid for Runtime.
32. As a designer, I want invalid property combinations explained at the selected node, so that I can fix errors where they occur.
33. As a designer, I want accessibility labels, roles, focus order, target size, contrast, and reduced-motion behavior to be authorable and validated, so that accessibility is part of design rather than a final audit.
34. As a designer, I want preview to use the same native component semantics as Runtime, so that the editor does not promise behavior the application lacks.
35. As a designer, I want interactive preview and design modes to be distinct, so that interacting with a prototype does not accidentally edit it.
36. As a designer, I want declarative local state and conditions, so that prototypes can express meaningful application behavior without arbitrary scripts.
37. As a designer, I want typed repeated-content bindings, so that lists and grids can render Studio Library collections.
38. As a designer, I want typed forms and validation, so that input flows can be prototyped and compiled safely.
39. As a designer, I want typed overlays, dialogs, sheets, popovers, toasts, and tooltips, so that transient UI remains Runtime-aligned.
40. As a designer, I want declarative actions for navigation, state changes, data operations, and approved host capabilities, so that interactions remain inspectable.
41. As a designer, I want an interaction graph and event inspector, so that behavior can be understood without reading generated code.
42. As a designer, I want diagnostics to link back to screens, nodes, bindings, interactions, assets, or extensions, so that failures are actionable.
43. As a designer, I want immutable revision history with named undo groups, so that I can understand and reverse meaningful changes.
44. As a designer, I want undo and redo to cover user, agent, MCP, ingestion, and extension edits uniformly, so that no author can bypass history.
45. As a designer, I want failed atomic edits to leave the prior revision unchanged, so that partial mutations cannot corrupt a project.
46. As a designer, I want local autosave and crash recovery, so that unexpected termination does not lose accepted edits.
47. As a designer, I want to restore a project from a logical snapshot and operation journal, so that recovery is independent of a physical database directory.
48. As a designer, I want cloud saving to be optional, so that offline-only projects remain fully usable.
49. As a designer, I want to enable cloud synchronization later, so that a local project can become available on my other devices.
50. As a designer, I want same-user multi-device synchronization, so that I can continue work from another desktop.
51. As a designer, I want explicit conflict explanations and resolution choices, so that concurrent offline edits never disappear through last-writer-wins behavior.
52. As a designer, I want disabling sync to stop transfers without disabling local editing, so that cloud participation remains reversible.
53. As a designer, I want account revocation and expired credentials to leave local work intact, so that authentication failures do not destroy projects.
54. As a designer, I want Studio Library assets to have stable identities, metadata, provenance, and content hashes, so that references survive renames and deduplicate safely.
55. As a designer, I want to add common image, video, audio, font, document, and icon formats, so that the Library supports ordinary design work.
56. As a designer, I want originals preserved and deterministic Runtime variants generated, so that source quality and packaged compatibility are both maintained.
57. As a designer, I want unsupported codecs or unsafe SVG content diagnosed, so that media failures do not appear only after launch.
58. As a designer, I want typed Content Collections with schema-aware records and operations, so that sample and packaged application data are easy to manage.
59. As a designer, I want Content Bindings to assets, records, fields, and repeated collection items, so that data use remains typed and traceable.
60. As a designer, I want fixture states for empty, loading, populated, error, and edge cases, so that designs can be evaluated beyond the happy path.
61. As a designer, I want Library content and assets packaged into an offline snapshot, so that the Runtime Application does not depend on Designer availability.
62. As a designer, I want unused packaged assets identified, so that I can control application size without deleting Library sources.
63. As an agent, I want scoped read access to the active project, selection, diagnostics, Library, and command schemas, so that I can make relevant edits without unrestricted host access.
64. As an agent, I want to apply typed command batches directly to the active Studio Design, so that changes appear in real time.
65. As an agent, I want command preconditions and validation results, so that I can recover from stale context instead of overwriting newer work.
66. As an agent, I want one task to become a named undo group even when it contains many commands, so that the user can reverse my intent coherently.
67. As an agent, I want progress, accepted operations, warnings, and failures visible in the Designer, so that the user understands what is happening.
68. As a designer, I want to cancel an active agent task, so that no further batches are accepted after cancellation takes effect.
69. As a designer, I want to keep editing while an agent works, so that agent assistance does not impose a proposal gate.
70. As a designer, I want conflicts between my edits and an agent's stale commands surfaced without losing either intent, so that live collaboration with agents remains safe.
71. As an MCP client, I want the same scoped command and query interfaces as other agents, so that MCP cannot become a privileged mutation path.
72. As a designer, I want agents to interpret external files as reference mockups, so that common source material can seed a Studio Design without native format importers.
73. As a designer, I want agent-led ingestion to retain source provenance, so that generated design content can be traced to its reference.
74. As an extension author, I want to declare extension identity, version, compatibility, and requested capabilities, so that installation can be validated before activation.
75. As an extension author, I want to contribute primitive compositions, inspector declarations, commands, actions, content types, validators, and migrations, so that extensions can add full authoring workflows.
76. As an extension author, I want lifecycle hooks with explicit inputs and outputs, so that activation, project open, validation, build, migration, and shutdown remain deterministic.
77. As an extension author, I want to request host-mediated capabilities, so that useful integrations do not require native code or unrestricted networking.
78. As a designer, I want extension capability requests explained and consented to, so that installing an extension does not silently broaden authority.
79. As a platform operator, I want third-party extensions prevented from adding native renderers, raw GPUI access, HTML/CSS injection, or arbitrary drawing, so that the closed Runtime contract remains authoritative.
80. As a platform engineer, I want every protocol component kind to have a semantic native renderer and Designer inspector, so that catalog declaration means real platform support.
81. As a platform engineer, I want component readiness tracked across schema, native mapping, rendering, editing, Runtime verification, and release certification, so that fallback rendering cannot be mistaken for completion.
82. As a platform engineer, I want POS-specific renderer conventions removed from the generic path, so that other applications do not inherit hard-coded behavior.
83. As a platform engineer, I want Runtime Projection to preserve stable design identities where the protocol permits, so that diagnostics and preview updates map back to source nodes.
84. As a platform engineer, I want deterministic projection and packaging, so that identical revisions produce byte-identical unsigned build inputs and reproducible signed contents.
85. As a designer, I want build readiness to report errors and warnings before packaging, so that launch failures are predictable.
86. As a designer, I want to build, sign, launch, and inspect a Runtime Application from Designer, so that the complete journey remains inside Studio Platform.
87. As a Runtime Application author, I want typed collection helpers, so that application code can use embedded data without constructing raw database queries.
88. As a Runtime Application author, I want an explicitly declared bounded Surreal query capability when helpers are insufficient, so that advanced data access remains possible under host policy.
89. As a Runtime Application user, I want each application isolated in its own Runtime-managed data namespace, so that applications cannot read or alter each other's data.
90. As a Runtime Application user, I want application data to persist across launches, so that locally useful applications are not limited to fixtures.
91. As a Runtime Application author, I want signed data migrations with a dedicated lifecycle, so that schema evolution occurs before ordinary application access.
92. As a Runtime Application author, I want schema-constrained REST actions, so that an application can call approved endpoints without receiving a general network socket.
93. As a Runtime Application author, I want host-owned WebSocket sessions with typed messages, limits, and lifecycle events, so that real-time features remain capability controlled.
94. As a Runtime Application author, I want host-mediated OAuth, so that provider tokens never enter guest memory or project files.
95. As a Runtime Application author, I want host-mediated Stripe test payments, so that payment credentials and confirmation remain protected by Runtime.
96. As a platform operator, I want provider credentials, OAuth tokens, sockets, database handles, and filesystem handles withheld from guests, agents, MCP clients, and extensions, so that authority remains in the host.
97. As a platform operator, I want network destinations, request schemas, methods, message sizes, rates, timeouts, redirects, and response shapes bounded by signed declarations, so that REST and WebSocket access is auditable.
98. As a platform operator, I want diagnostics, logs, histories, backups, and sync payloads redacted, so that secrets cannot leak through supporting systems.
99. As a release engineer, I want the complete POS reference journey to run against real official sandbox endpoints, so that simulated success is not mistaken for integration evidence.
100. As a release engineer, I want offline, recovery, synchronization, migration, accessibility, security, deterministic build, and performance evidence, so that Studio Designer cannot ship on visual completeness alone.
101. As a release engineer, I want future web and mobile Designer hosts to reuse the domain and projection contracts, so that the desktop implementation does not trap the model in GPUI.
102. As a designer, I want a clean agent welcome surface before a conversation begins, so that starting with an agent feels focused rather than like opening another editor panel.
103. As a designer, I want Studio's company mark on the left and the selected model control on the right of the agent composer, so that product identity and execution choice remain clear without clutter.
104. As a designer, I want to search and select among configured compatible models, so that I can choose the agent appropriate for the task.
105. As a designer, I want the selected model recorded with each agent run, so that history and diagnostics explain which model produced a result.
106. As a designer, I want sending the first message to clear the welcome surface and restore the design workspace, so that the conversation becomes part of authoring rather than replacing it.
107. As a designer, I want the active Agent Conversation to continue in a floating window, so that I can follow and direct the agent while viewing the design it changes.
108. As a designer, I want the floating Agent Conversation to preserve its thread, model, context, progress, and position while I switch editor views, so that changing workspace layout does not interrupt the task.
109. As a designer, I want an Import action in the agent composer, so that I can provide admitted files and reference material for Agent-led Ingestion without mounting an unrestricted folder.
110. As a designer, I want the agent composer to omit terminal switching and “full access” controls, so that the surface communicates Studio's scoped command authority accurately.
111. As a designer, I want agent messages to contain inline Agent References to screens, nodes, compositions, assets, content, properties, interactions, diagnostics, revisions, and imported sources, so that the agent can point to exact design context.
112. As a designer, I want Agent References to show recognizable type icons and concise labels, so that I can scan a response without parsing raw identifiers or paths.
113. As a designer, I want to activate an Agent Reference and open or select its target in the appropriate Designer surface, so that conversation and authoring context remain directly connected.
114. As a designer, I want stale or unavailable Agent References to remain visible with an explanation, so that historical messages never silently point somewhere else.

## Implementation Decisions

### Product and ownership

- Studio Designer is a first-party desktop application implemented in Rust with a native GPUI interface. Desktop is the only Designer host in this specification.
- Studio Designer lives in the Studio Platform monorepo and extends the existing Runtime rather than maintaining a forked protocol or renderer.
- One Studio Project owns one Studio Design and produces one Studio Runtime Application.
- Studio Design, Studio Library, project configuration, extension declarations, revisions, and build metadata are project-owned. Runtime-owned user data is not part of Studio Design.
- Studio Designer replaces Studio Canvas's intended first-party authoring role. Instatic informs editor mechanics and interaction patterns but is not ported wholesale and is not a source-format dependency.

### Primary authoring seam

- Introduce a deep `DesignerSession` module as the primary interface used by the native shell, agents, MCP, tests, and build orchestration.
- A session owns the active project revision, selection, screen, device profile, tool, panel state, diagnostics, command registry, history cursor, agent runs, persistence coordination, and optional sync status.
- Callers submit typed queries and command batches and receive immutable snapshots, command receipts, progress, diagnostics, and conflict results. Callers never mutate stored entities directly.
- Focus View and Workbench View consume the same session snapshots and commands. View switching changes presentation only.
- GPUI types, SurrealDB types, cloud transport types, and Runtime `UiNode` types do not appear in the Studio Design interface.

### Studio Design source model

- Store visual source as a flat map keyed by opaque stable node identities, with ordered child identities and a validated parent index. Derive nested editor and Runtime trees rather than persisting them as the source.
- A project contains screens, routes, Primitive Nodes, Reusable Composition definitions and instances, tokens, responsive variants, typed interactions, fixture states, Studio Library references, provenance, schema version, and immutable revision metadata.
- Primitive Node kinds must come from the approved Runtime component catalog. `Box` is the generic container. Unknown kinds fail validation and compilation.
- Reusable Compositions are project-owned trees of Primitive Nodes with typed inputs, defaults, admitted overrides, slots, and versioned instances. They do not create new Runtime renderer kinds.
- Overlay and absolute placement are typed layout properties of supported primitives, especially Stack children. A sale treatment over an image is expressed by ordinary primitives and placement properties, not a Designer-only `SaleBadge` kind.
- Layout, style, tokens, bindings, interactions, accessibility, and responsive metadata use closed versioned schemas with unknown-field rejection.
- Responsive values have a base plus explicit overrides at supported breakpoints. Device profiles contribute viewport and input/environment metadata but do not fork the design document.
- Runtime `UiNode` is an output of Runtime Projection, never the authoring source.

### Command algebra, revisions, and history

- Every user, agent, MCP, ingestion, and extension edit enters the same command engine.
- Commands include operation identity, actor identity and kind, project identity, base revision, schema version, typed payload, structural/property preconditions, and enough prior information to produce an inverse.
- Commands cover project and screen lifecycle; node insertion, movement, reorder, replacement, duplication, deletion, and restoration; property, token, responsive, accessibility, binding, interaction, composition, Library, fixture, extension, and project-setting changes.
- An atomic command batch either validates and commits completely or produces no new revision.
- Each accepted batch creates an immutable revision, a deterministic command receipt, an outbox record when sync is enabled, and a history entry. Multiple streamed agent batches may share one named undo-group identity.
- Undo applies validated inverse commands as a new revision rather than moving storage backward. Redo reapplies the original intent against explicit preconditions.
- Stable identities survive rename, reparent, reorder, styling, and responsive edits. Deletion produces tombstone information sufficient for undo, sync conflict detection, and reference diagnostics.
- User edits may continue during agent work. Stale preconditions yield structured conflicts; they never trigger silent last-writer-wins mutation.

### Native editor experience

- Focus View is the default workspace. It prioritizes the canvas, floating tools, contextual right inspector, and a compact bottom activity/history dock. The Agent Conversation itself becomes a floating window after its initial welcome state.
- Workbench View provides persistent screens and hierarchy, Studio Library, canvas controls, inspector, diagnostics, interactions, agent activity, and history.
- Both views expose Design, Prototype, and Data destinations and every authoring capability. Focus View uses collapsible left, right, and bottom panels plus the command bar for surfaces that Workbench keeps visible.
- Switching views preserves active project, screen, selection, profile, canvas transform, tool, command state, agent run, history, diagnostics, and unsaved work. Only panel geometry and collapse state are view-specific.
- Canvas interaction is driven by Studio Design identities and commands. Hit testing, selection overlays, drag handles, guides, placement previews, and inspector updates do not write directly to Runtime trees.
- Design mode edits the source. Prototype mode dispatches declared interactions against an isolated preview state and cannot mutate the design unless the user invokes an explicit authoring command.
- The accepted throwaway prototype establishes the structural direction: variant B is Focus View, variant A is Workbench View, and variant C's journey/build-readiness concepts may become panels rather than a third primary workspace.

### Agent welcome, composer, and conversation

- Entering the Agent destination with no active messages presents a clean welcome surface centered on a single spacious composer. Studio's company mark sits on the left side of the composer chrome and the active agent model control sits on the right.
- The initial composer contains the prompt field, scoped context/attachment affordance, Import, model selection, and send. It does not expose “Switch to Terminal,” “Full access,” folder mounting, or other controls that imply authority outside Studio Designer.
- Import opens the host-owned Agent-led Ingestion flow. Selected files or directories are admitted as scoped source material with provenance; they do not grant the agent continuing arbitrary filesystem access. The UI uses **Import**, not **Add folder**.
- The model selector is a host-owned searchable popover listing configured providers and Studio-compatible models with availability and relevant model metadata. Provider credentials remain outside the conversation and agent process.
- A selected model applies to the next run and is recorded with that run, its assistant messages, diagnostics, command batches, and undo group. Changing models affects later runs without rewriting prior provenance.
- Sending the first message dismisses the welcome composition, reveals the active design workspace, and moves the same Agent Conversation into a floating window. The thread is not restarted or summarized merely because its presentation changes.
- The floating Agent Conversation is movable, resizable, collapsible, and constrained to the Designer workspace. Its placement and size are presentation state; its thread, model/run provenance, context, progress, and results belong to the project session.
- View switching preserves the floating conversation. Agent activity and history may also appear in the compact bottom dock, but the activity feed and Agent Conversation are distinct presentations: the former reports operations, while the latter contains the interactive thread.
- The composer remains deliberately minimal after transition. Any later control must represent a specific Studio capability rather than a generic coding-agent permission mode.
- Agent messages are structured content, not only Markdown strings. They may contain text, progress, diagnostics, and inline Agent References resolved through `DesignerSession`.
- An Agent Reference carries a typed target kind, stable target identity, source revision or imported-source identity when relevant, and a safe display hint. Raw paths and display labels are not identity.
- Supported reference targets include projects, screens, Primitive Nodes, Reusable Composition definitions and instances, Studio Library Assets, Content Collections and fields, bindings, tokens, inspector properties, interactions, diagnostics, commands, operations, revisions, build artifacts, and admitted imported sources.
- Agent References render as compact accessible chips with a type-specific icon and current concise label. They support keyboard focus, activation, hover/focus description, copyable safe identity, and appropriate stale or denied states.
- Activating a resolved Agent Reference performs navigation or selection only: it opens the target screen, selects the node, reveals the Library item, focuses the inspector property, opens the interaction or diagnostic, or shows the referenced revision/build record. It never mutates Studio Design by itself.
- Reference resolution uses stable identity against the referenced revision and current session. A renamed target displays its current label while retaining identity; a deleted, unavailable, permission-denied, or imported-source-missing target remains a non-destructive chip with an explicit explanation.

### Component catalog and responsive coverage

- V1 covers every kind in the approved Runtime catalog; components are not cherry-picked for Designer.
- A component is complete only when it has a closed property schema, semantic native renderer, Designer insertion metadata, component-specific inspector, events and state behavior, keyboard/pointer/touch behavior where applicable, accessibility semantics, responsive behavior, preview/Runtime parity, and automated evidence.
- Generic fallback rendering does not satisfy completion. POS-specific identifiers and behavior must be removed from the generic render path.
- Maintain a capability matrix with separate states for protocol declaration, native mapping, semantic native rendering, Designer editability, Runtime verification, and release certification.
- Support phone, foldable, tablet, laptop, desktop, ultrawide, television, and 4K profiles with base, 480, 640, 768, 1024, 1280, 1536, 1920, 2560, and 3840 breakpoints.
- Preserve profile metadata for orientation, pixel ratio, safe areas, touch, pointer, hover, keyboard, remote focus, and reduced motion. Designer previews all profiles; Runtime certification is recorded per available native host.

### Studio Library

- Studio Library is the project-owned catalog of Assets, Content Collections, Content Bindings, fixture states, schemas, and provenance used by designers, agents, extensions, and the packaged Runtime Application.
- Assets have opaque stable identity, content hash, media kind, original format, metadata, provenance, created/updated revision, original blob reference, normalized variants, usage references, and packaging policy.
- Blob content is stored in a content-addressed local asset store; SurrealDB stores identity, metadata, hashes, relationships, and synchronization state. Blob transfer and deduplication are independent from Design operation synchronization.
- Preserve source originals. Generate deterministic Runtime variants according to the approved host decoder matrix. Sanitize SVG and reject unsafe active content.
- Baseline accepted originals include PNG, JPEG, WebP, GIF, AVIF, sanitized SVG; MP4, WebM, MOV; MP3, WAV, Ogg, FLAC, M4A/AAC; WOFF2, WOFF, TTF, OTF; PDF, plain text, Markdown; and SVG or raster icons. Admission does not imply every Runtime host can render every original; unsupported use produces diagnostics.
- Content Collections use versioned typed schemas, stable record identities, validation, indexes declared by the project, and typed create/read/update/delete operations.
- Content Bindings are typed references to an Asset, collection, record, field, current repeated item, or fixture value. Broken or type-incompatible references are build errors unless the binding declares a valid fallback.
- Fixture states are authoring data used for preview and tests. Runtime packages receive a deterministic offline Library snapshot containing only admitted content and required asset variants.
- Deletion is reference-aware. Referenced assets, records, fields, or schemas require replacement, unbinding, or an explicit breaking change that leaves diagnostics.

### Declarative interactions

- Store interactions in a typed graph separate from the visual node map while referencing stable screen, node, binding, state, action, and capability identities.
- Supported triggers come from approved component and lifecycle events. Supported effects include navigation, local state updates, conditions, collection operations, form validation/submission, overlays, and approved host actions.
- Conditions and value expressions use a bounded typed expression model. Arbitrary JavaScript, HTML, CSS, database scripting, and unrestricted guest evaluation are not part of Studio Design.
- Prototype execution uses isolated ephemeral state and the same interaction semantics used by Runtime Projection.
- The editor exposes interaction traces and source-linked diagnostics for invalid triggers, cycles, missing targets, incompatible values, and unavailable capabilities.

### Live agents, MCP, and agent-led ingestion

- Agents and MCP clients connect through host-owned scoped session interfaces. They receive no database, filesystem, network, GPUI, compiler-process, credential, or Runtime guest handles.
- Read scopes may include project summaries, selected subtrees, schemas, Library metadata or requested blobs, interaction graphs, diagnostics, command schemas, and revision history. Scope is explicit and auditable.
- Mutations are typed command batches with actor attribution, base revision, preconditions, progress metadata, and undo-group identity.
- Accepted agent operations appear in the live design immediately. A proposal gate is not required.
- Cancellation prevents acceptance of later batches from that run; previously accepted batches remain in their undo group and can be undone normally.
- Agent failures preserve accepted revisions, reject incomplete atomic batches, and surface safe diagnostics. Retrying uses idempotent operation identities.
- Concurrent user edits are permitted. The command engine admits independent work and returns structured conflicts for stale or overlapping work.
- Agent-led Ingestion interprets external images, PDFs, design files, markup, or other references and constructs Studio Design through the same command interface. No dedicated Rust importer is required for external design formats in v1.
- Ingested nodes, assets, and content retain source provenance and are validated exactly like manually authored content.
- Agent Conversations persist structured messages, model/run provenance, imported context, Agent References, and safe progress/result metadata. They do not persist provider secrets, unrestricted prompts containing protected values, or raw capability handles.

### Extension framework

- Extensions are sandboxed, versioned packages admitted by a first-party Extension Registry.
- An extension descriptor declares identity, publisher, version, compatible Studio and schema versions, contributions, migrations, requested capabilities, and integrity information.
- Extensions may contribute Reusable Compositions of approved Primitive Nodes, inspector declarations made from first-party editor controls, commands, declarative actions, Content Collection types, validators, migrations, templates, and capability-mediated integrations.
- Extensions cannot add native renderer kinds, execute unrestricted native code, access raw GPUI, inject HTML/CSS, draw arbitrary surfaces, access SurrealDB directly, or open arbitrary sockets.
- Extension lifecycle includes admission, installation, activation, project open, validation, build participation, migration, deactivation, and removal. Each hook has bounded input, output, time, memory, and failure behavior.
- Extension commands still pass through `DesignerSession`; extension migrations run through a separately authorized project migration path and create recovery points.
- Requested capabilities are deny-by-default, explained to the user, recorded per project, and revocable. Removal reports remaining nodes, bindings, content, actions, or migrations owned by the extension before changing the project.

### Designer persistence and optional cloud synchronization

- Embed exactly pinned SurrealDB 3.2.4 with RocksDB for the first shipping Designer store, subject to the required packaging, recovery, performance, licensing, and security qualification. Keep the engine behind a host-owned `LocalStore` interface.
- The local store materializes Studio Design, Studio Library metadata, immutable revisions, operation outbox, sync receipts and cursors, conflicts, extension state, and schema metadata. Large media remains in the content-addressed asset store.
- Each accepted edit transaction atomically updates materialized state and appends its revision/outbox records using an explicit durability configuration.
- Guests, extensions, agents, and MCP clients receive typed host interfaces only; they never receive a Surreal handle or SurrealQL access.
- Cloud sync is an application-owned typed-operation protocol through an authenticated Studio Cloud API backed by hosted SurrealDB and object storage. Embedded and hosted SurrealDB storage replication, database changefeeds, and physical files are not the sync protocol.
- An uploaded operation contains idempotent operation identity, device, account actor, project, base revision, ordered command payload, schema/protocol version, content hashes, and creation metadata.
- The cloud service validates identity, project ownership, schema version, preconditions, command invariants, and asset admission; assigns a monotonic per-project server revision; stores the accepted operation; updates its materialized view; and returns an idempotent receipt.
- Clients pull accepted operations after a server cursor and apply them through the same command engine. Cursor and materialized state update atomically.
- Explicit algebra may rebase independent operations. Same-property edits, delete-versus-edit, structural conflicts, failed preconditions, schema skew, and unavailable assets become recoverable conflict records. Silent last-writer-wins is forbidden.
- Periodic logical Studio Design snapshots bound replay time. Logical snapshots and operation journals are portable recovery artifacts; live physical database directories are not project backups.
- V1 synchronization is same-user multi-device only. Enabling sync creates cloud identity from a declared local revision; disabling it stops transfer while preserving local authority and unsent operations.
- Account tokens use the operating-system credential facility. Hosted database credentials remain server-side and no secret enters Design, Library metadata, history, sync payloads, diagnostics, or backups.
- Schema migrations are numbered and forward-only with pre-migration recovery, transactional execution where supported, post-migration validation, and cloud expand/cutover/contract compatibility. Engine upgrades are rehearsed separately from Studio schema migrations.
- SurrealKV remains a qualification candidate, not the v1 persistence default.

### Runtime Projection, preview, and packaging

- Introduce a deterministic Runtime Projection module that accepts an immutable Studio Design revision and Library snapshot and returns validated Runtime trees, events, actions, route/application metadata, packaged asset inputs, source maps, warnings, and errors.
- Projection resolves Reusable Compositions, responsive values, tokens, bindings, fixture or packaged content, interactions, extension contributions, and capability declarations without mutating the source revision.
- Source maps preserve relationships from generated Runtime nodes and diagnostics back to Studio Design identities.
- Preview mounts projection output through the approved Runtime catalog, retained UI registry, native state store, and host action interfaces. Designer-only rendering behavior is forbidden.
- Incremental preview may cache and patch projection output, but full projection of the same revision is the correctness oracle.
- Build performs schema validation, reference validation, extension admission, asset/media validation, component readiness checks, accessibility checks, interaction validation, capability admission, deterministic projection, Runtime module generation, Library snapshot construction, package integrity, and signing.
- The compiler may introduce a typed Studio intermediate representation and AssemblyScript/Wasm lowering behind the projection interface. Compiler internals do not become the Studio Design source contract.
- The output is a signed `.studio` package consumed by Studio Runtime. No HTML, website, mobile bundle, or third-party runtime export is required.
- Identical source revision, toolchain, declared build inputs, and signing configuration produce deterministic package contents except for explicitly isolated signature material.

### Runtime prerequisites and application capabilities

- Complete the generic semantic native renderer for every approved catalog kind before claiming full Designer coverage. Remove POS-specific identifiers and branches from generic rendering.
- Extend package and protocol contracts to carry the admitted Library snapshot, capability declarations, data schema/migrations, REST schemas, WebSocket schemas, OAuth provider declarations, and payment declarations required by the generated application.
- Studio Runtime owns one logical embedded SurrealDB Runtime Data Store. Physical storage layout remains an implementation detail and need not be one literal file.
- Runtime derives an Application Data Namespace from verified publisher/application identity and prevents guest-selected namespace or database changes.
- Expose typed `data.collection` operations through generated SDK helpers. Applications may opt into bounded `data.surreal.query` only through a signed capability declaration.
- Bounded Surreal queries are parameterized, execute only inside the assigned Application Data Namespace, reject namespace/database switching, and cannot reach Studio system data, filesystem, database scripting, or database-initiated networking.
- Signed application data migrations execute through a host-owned pre-launch lifecycle with backup, version checks, idempotency, rollback/recovery policy, and separate authority from normal data operations.
- REST access is a host-owned request broker. Signed declarations constrain origins, paths, methods, headers, request and response schemas, sizes, redirects, timeouts, retries, rates, and redaction.
- WebSocket access is a host-owned session broker. Signed declarations constrain endpoint, subprotocol, authentication reference, outbound/inbound message schemas, sizes, rates, reconnect behavior, lifetime, and lifecycle events. Guests receive session identities and typed events, never sockets.
- OAuth is host-owned. Runtime performs browser/redirect or device flows, stores refresh/access tokens in protected host storage, and exposes only approved identity claims, status, and provider action results to the application.
- Stripe is host-owned. Runtime uses official test mode for acceptance, retains trusted confirmation and secret handling, and exposes typed payment intents/results rather than provider credentials or unrestricted Stripe access.
- Capability admission extends the existing principal, trust, action-gate, redaction, Wasm budget, and no-WASI model. Denied or unavailable capabilities fail safely with source-linked diagnostics.

### Trust domains

- The first-party Designer host owns Studio Design authority, LocalStore, asset store, compiler orchestration, account credentials, sync worker, extension admission, agent/MCP scopes, native shell, and build signing coordination.
- Studio Cloud owns account/project authorization, hosted credentials, canonical accepted-operation order, cloud snapshots, conflict admission, and remote asset storage.
- Agents and MCP clients are untrusted command authors. Sandboxed extensions are untrusted contribution authors. Generated Runtime guests are untrusted application logic. None may cross host interfaces directly.
- Runtime owns application persistence, provider credentials, OAuth tokens, network connections, protected confirmation surfaces, secret inputs, capability enforcement, and guest termination.
- All external values are validated at the receiving seam. Diagnostics expose stable safe codes and redacted context, not secrets or raw provider/database errors.
- Local database and asset directories use owner-only permissions and rely on the documented supported operating-system disk-encryption posture unless a separately reviewed encryption layer is added.
- SurrealDB's exact license and transitive notices require commercial release review. Studio must remain an application, not expose customer-controlled general database administration, and must ship required notices and an SBOM.

### Reference application and delivery

- Extend the existing POS example into the canonical authoring-to-Runtime acceptance project.
- The project includes catalog, product detail, cart, checkout, receipt, and recovery screens; Reusable Compositions; responsive profiles; Library assets and typed product content; bindings; navigation; local state; repeated content; forms; overlays; agent edits; undo; persistence; cloud synchronization; extensions; preview; build; signing; and Runtime launch.
- Runtime acceptance adds real embedded application data, a schema-constrained REST request, a host-owned WebSocket session, an official OAuth sandbox flow, and Stripe test-mode payment.
- Runtime prerequisite tickets may be implemented before Designer tickets. Their contracts must remain aligned with Runtime Projection and the final POS journey.
- After this specification is approved, `/to-tickets` must split delivery into dependency-ordered vertical tracer bullets. Each ticket must deliver demonstrable behavior through the confirmed seams rather than one horizontal infrastructure layer.

## Testing Decisions

### Testing philosophy

- Tests verify externally observable behavior through public interfaces, not implementation structure, private fields, database table layout, or incidental GPUI hierarchy.
- Every defect fix begins with a test capable of reproducing the user-visible failure. Every feature proceeds in small red-green vertical slices.
- Pure command, validation, admission, serialization, conflict, retry, redaction, projection, and deterministic-build logic uses deterministic tests with independently derived expected results.
- Fakes may support isolated pure-domain tests, but they do not count as integration evidence for databases, cloud synchronization, provider APIs, OAuth, REST, WebSockets, packaging, or Runtime launch.

### Primary DesignerSession seam

- The primary functional suite creates a real `DesignerSession`, submits only public queries and commands, and asserts immutable Studio Design snapshots, command receipts, revisions, named undo groups, diagnostics, conflicts, persistence, and sync status.
- Tests cover every command family, atomic rollback, stable identity, undo/redo, composition propagation, responsive overrides, bindings, interactions, extension contributions, agent streaming, cancellation, user/agent concurrency, stale preconditions, Agent Conversation persistence, model/run provenance, Agent Reference resolution, and migration.
- Persistence tests use real embedded SurrealDB with temporary RocksDB directories. They cover create, reopen, autosave, forced termination, recovery, logical export/restore, operation replay, corrupted or incompatible metadata, and migration recovery.
- Synchronization tests use two or more real sessions and the real Studio Cloud protocol backed by a real test SurrealDB service. They cover duplicate delivery, reorder, disconnect during upload, accepted-but-response-lost, stale bases, independent rebase, property conflict, structural conflict, deletion, asset resume/deduplication, schema skew, token expiry, account revocation, and disabling/re-enabling sync.
- Studio Library tests verify content hashing, deduplication, provenance, original preservation, deterministic variants, SVG sanitization, schema validation, binding type checks, reference-aware deletion, fixtures, package selection, and restore.

### Native Designer shell seam

- Launch the real GPUI Studio Designer and exercise visible commands and controls in both Focus and Workbench views.
- Verify that view switching preserves session state and that every persistent Workbench surface is reachable from Focus View.
- Exercise mouse, keyboard, focus traversal, command bar, hierarchy editing, drag/drop, resizing, guides, inspector edits, responsive profiles, prototype interactions, diagnostics, history, live agent progress, cancellation, undo, preview, and build readiness.
- Verify the clean agent welcome surface, company-mark/model-control placement, searchable model selection, minimal composer, Import flow, absence of terminal/full-access controls, first-message transition, floating conversation movement/resizing/collapse, and preservation across Focus and Workbench views.
- Verify every Agent Reference target kind through mouse and keyboard activation, including correct selection/panel navigation, current-label resolution after rename, historical revision context, and safe stale, missing, or denied states.
- Accessibility acceptance includes meaningful roles and names, complete keyboard operation, predictable focus, focus restoration, target sizes, contrast, reduced-motion equivalence, screen-reader-visible validation, and profile-specific input semantics.
- Visual evidence may use deterministic screenshot comparisons for stable overlays, frames, and selection chrome, but semantic and interaction assertions remain authoritative.

### Authoring-to-Runtime seam

- Starting from the canonical POS Studio Project, use the public Designer build command to validate, project, package, sign, launch, and exercise the real Runtime Application.
- Assert multi-screen navigation, state, collections, forms, overlays, component events, accessibility, responsive variants, offline Library content, persisted Runtime data, signed migrations, failure recovery, and source-linked diagnostics.
- Execute Stripe against official test mode, OAuth against an official provider sandbox/test application, REST against the approved real test endpoint, WebSocket against the approved real test endpoint, and Runtime data against real embedded SurrealDB.
- Credentials are supplied through protected test/release configuration and never committed, printed, snapshotted, placed in guest memory, or persisted in project artifacts.
- Live integration suites may be credential-gated during ordinary local development, but release acceptance cannot pass without running them successfully. Simulators do not substitute for this gate.

### Existing supporting seams

- Extend protocol fixture and compatibility tests for every new closed schema, unknown-field rejection, limits, stable error codes, version negotiation, and malicious payload.
- Extend retained registry tests for atomic mounts/patches, source identity preservation, targeted updates, parent relationships, limits, and rollback.
- Extend component catalog tests so every declared kind proves schema, semantic native mapping, editor metadata, interaction behavior, accessibility, and no generic fallback.
- Extend package tests for deterministic contents, Library snapshots, capability declarations, data migrations, path traversal resistance, signature verification, tamper detection, and size/count limits.
- Extend Wasm and security tests for no-WASI isolation, budgets, capability denial, opaque handles, secret lifecycle, redaction, REST/WebSocket admission, OAuth/payment mediation, and safe termination.
- Extend application-shell integration tests for Runtime Data Store isolation, migrations, network brokers, protected confirmation, navigation recovery, compositor loss, and shutdown cleanup.

### Required non-functional evidence

- Component coverage is release-blocking until every approved kind reaches semantic native rendering, Designer editability, Runtime verification, and applicable host certification.
- Determinism is release-blocking: repeated projection and package builds from identical admitted inputs must match at the declared deterministic layers.
- Durability is release-blocking: no accepted command may disappear after the documented durability point, and crash recovery must return the last durable revision without a partially committed batch.
- Sync correctness is release-blocking: every accepted operation is applied exactly once logically, conflicts preserve both intents, and no tested disconnect/retry sequence loses acknowledged or unsent work.
- Security is release-blocking: capability bypass, cross-application data access, raw credential/token/socket/database-handle exposure, unredacted persistence, unsafe SVG, path traversal, invalid signature admission, and extension escape tests must all fail closed.
- Accessibility is release-blocking for the native Designer and canonical Runtime journey, with remaining human assistive-technology review recorded as required release evidence.
- Performance benchmarks cover clean build, binary/package size, cold launch, project open, common command latency, canvas interaction, projection, incremental preview, full build, 10,000-operation replay, sync catch-up, memory use, and embedded-store compaction on the supported baseline machine.
- Before release, the project records baseline hardware and explicit budgets for those benchmarks; continuous integration fails regressions beyond the approved tolerance. A benchmark without an approved budget is evidence collection, not certification.
- Recovery rehearsal covers logical backup/restore, Studio schema migration, SurrealDB patch upgrade, deliberately incompatible engine fixture, failed extension migration, interrupted asset transfer, and application data migration failure.
- Run the repository's locked workspace tests, strict Clippy, formatting check, release build, fuzz targets, dependency audit, SBOM generation, license review, and signed-package verification as release gates.

## Out of Scope

- Web or mobile Studio Designer hosts in this effort.
- Runtime execution certification on hosts that do not yet exist, while retaining portable contracts and packaged variants for future hosts.
- Export to HTML, CSS, React, Svelte, Flutter, mobile application bundles, websites, Figma, or any runtime other than Studio Runtime.
- Dedicated Rust importers for HTML, design tools, images-as-layout, or other external design formats; agents perform Agent-led Ingestion.
- Treating HTML, `UiNode`, Wasm, AssemblyScript, generated code, or the projected Runtime tree as the editable source of truth.
- Arbitrary JavaScript, database scripting, HTML/CSS injection, unrestricted native plugins, raw GPUI access, arbitrary rendering, or third-party native component implementations.
- Direct guest, agent, MCP, or extension access to SurrealDB, provider credentials, OAuth tokens, sockets, filesystems, or cloud database credentials.
- Terminal switching, unrestricted “full access” agent modes, arbitrary folder mounting, or a general-purpose coding-agent console inside Studio Designer.
- General-purpose customer database administration or a database-as-a-service product.
- Simultaneous multi-user collaboration, invitations, organizations, presence, comments, multiplayer cursors, or human-to-human real-time coediting in v1.
- Public website CMS, public content publishing, or making Designer the live production database for generated applications.
- Production payment processing certification, production OAuth-provider approval, merchant onboarding, legal/compliance approval, and production cloud operations beyond the official sandbox acceptance required here.
- Automatic generation of arbitrary application business logic beyond the declared Studio interaction, data, extension, and Runtime capability contracts.
- SurrealKV as the shipping Designer persistence backend unless a later qualification decision replaces RocksDB.

## Further Notes

- Runtime prerequisite work is intentionally part of this specification. It may and often should land before the Designer feature that consumes it.
- The existing protocol, retained UI registry, component mapping, native state/event handling, security principals and capabilities, Wasm sandbox, signed package format, native shell, navigation, and POS journey are the reusable foundation. Their current existence is not evidence that the Designer-specific or generic renderer requirements are complete.
- The three confirmed acceptance seams are `DesignerSession`, the native Designer shell, and the complete authoring-to-Runtime journey. Supporting module tests provide diagnosis and contract confidence but cannot replace those seams.
- The accepted editor prototype is a primary UX reference under the Studio Designer Wayfinder artifacts. Its code is throwaway and must not be promoted into production architecture.
- The supplied agent-interface references establish the visual direction for the clean welcome composer, searchable model picker, floating post-message conversation, and icon-bearing inline Agent References. Studio adopts those interaction ideas without inheriting coding-agent terminal or unrestricted-access controls.
- Exact cloud hosting provider, OAuth sandbox provider, REST test service, WebSocket test service, baseline performance hardware, and release budgets must be recorded in the implementation tickets or release configuration before their dependent acceptance ticket can pass.
- This specification is intentionally broad because it defines the complete v1 journey. The next step is `/to-tickets`, which must preserve end-to-end tracer bullets and explicit blocking edges rather than translating each heading into a horizontal implementation ticket.
