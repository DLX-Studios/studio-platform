# Studio Designer authoring-to-Runtime specification

Status: ready-for-agent

## Problem Statement

Studio Platform has a secure native Runtime, a closed protocol, retained native UI infrastructure, signed application packages, and a POS reference application, but it does not yet provide a complete first-party visual authoring application. Studio Canvas explored the surrounding welcome, identity, onboarding, project-home, settings, and editor experience, while Instatic demonstrates effective node-oriented editing mechanics, but neither supplies the required Rust-native, offline-capable, Runtime-aligned Studio Designer journey.

Users need one complete desktop application in which they can pass through product welcome, local or cloud authentication, project discovery and management, visual design, prototyping, content population, validation, and Runtime build without treating HTML, `UiNode`, or a generated runtime tree as editable source. The application must preserve native Runtime semantics, support the complete approved component catalog, provide safe live agent editing, work offline through a Local Identity with optional cloud synchronization through a Cloud Identity, and prove the result by launching a signed application in the real Studio Runtime.

The existing Runtime also lacks several capabilities required by that journey. Its generic native rendering coverage is incomplete, its current render path contains POS-specific behavior, and it does not yet provide the Studio Design projection, Studio Library snapshot, embedded application data, maintained OAuth provider integrations, or bounded REST and WebSocket host network access required by the target reference application. Payment providers such as Stripe are reached through that bounded network access rather than a dedicated host capability. Those Runtime additions are prerequisites within this specification, even when they are implemented before the Designer itself.

## Solution

Build Studio Designer as a first-party Rust and GPUI desktop application inside Studio Platform. Its native application shell includes first-run welcome, an account chooser, Local Identity and Cloud Identity creation/login, cloud verification and setup, a complete Project Dashboard, settings and support surfaces, and safe transition into the existing authoring workspace. Each Studio Project owns one typed Studio Design and produces one Studio Runtime Application. Studio Design is a stable-ID, command-editable source model made from approved Primitive Nodes, project-owned Reusable Compositions, responsive layout and style metadata, design tokens, typed interactions, and Studio Library bindings.

All human, agent, MCP, ingestion, and extension changes pass through one validated `DesignerSession` interface. Atomic command batches create immutable revisions and undo groups, update the host-owned embedded SurrealDB store, and optionally enter a typed cloud synchronization outbox. The Designer provides a canvas-first Focus View by default and a persistent Workbench View for deep inspection; both are presentations of the same session and expose the same capabilities.

A Runtime Projection validates Studio Design and deterministically produces Runtime-compatible native trees, events, actions, Library snapshots, and signed `.studio` packages. Preview and packaged execution reuse the approved Runtime catalog and native adapters. The POS reference journey proves the complete path with real embedded data and official sandbox integrations mediated exclusively by the Runtime host.

## User Stories

1. As a designer, I want to create a Studio Project after unlocking a Local Identity, so that I can design without network access or a Cloud Identity.
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
94. As a Runtime Application author, I want first-party maintained OAuth provider plugins, so that enabling a provider gives me a correct, current authorization flow without embedding provider logic, secrets, or tokens in my application.
95. As a Runtime Application author, I want to complete Stripe test-mode payments through the schema-constrained REST broker, so that payment calls remain ordinary auditable network use while key-shaped values never leak into diagnostics or logs.
96. As a platform operator, I want provider credentials, OAuth tokens, sockets, database handles, and filesystem handles withheld from guests, agents, MCP clients, and extensions, so that authority remains in the host.
97. As a platform operator, I want network destinations, request schemas, methods, message sizes, rates, timeouts, redirects, and response shapes bounded by signed declarations, so that REST and WebSocket access is auditable.
98. As a platform operator, I want diagnostics, logs, histories, backups, and sync payloads redacted, so that secrets cannot leak through supporting systems.
176. As a platform engineer, I want OAuth provider integrations maintained as versioned declarative descriptors that update independently of application packages, so that provider drift never requires rebuilding authored applications.
177. As a developer, I want to select and configure integration plugins directly inside Studio Designer, so that credentials, capabilities, and routes are set up where I author instead of editing manifests by hand.
178. As an application operator, I want application-level users such as employees, technicians, and moderators, so that the people using my deployed app each have their own protected access.
179. As an application operator, I want roles bound to screens, actions, and individual data records, so that access control is enforced by the Runtime host rather than by interfaces merely hiding options.
180. As a restaurant operator, I want every point-of-sale station to share live table, check, and ticket state through a declared center server, so that four terminals never disagree about one dining room.
181. As an operator, I want stations to keep serving during network loss and reconcile afterward, so that connectivity failures never stop service or silently drop orders.
182. As a developer, I want to declare an application's data topology as device-local, a Studio Cloud-hosted center, or a self-hosted on-premises hub, so that one authored app deploys into different operational shapes without code changes.
183. As an application developer, I want declared inbound webhooks received by a host-owned listener, so that platform events reach my app through validated, bounded endpoints instead of exposed servers.
184. As an application operator, I want declarative scheduled and event-triggered background workflows, so that payroll accrual, escalation timers, and reports run without keeping a person logged in.
185. As an enterprise buyer, I want development, staging, and production environments isolating application data and secret values, so that testing never touches live business operations.
186. As an auditor, I want an append-only log of security-relevant application events such as authentication, authorization changes, destructive actions, and exports, so that accountability survives employee turnover.
187. As an operator of many installed stations, I want signed application updates delivered through a managed channel with staged rollout and rollback, so that updating four terminals does not mean visiting four machines.
188. As a template publisher, I want vertical templates whose brand surfaces are token-backed brand slots, so that a customer rebrands an entire operational app by swapping colors, type, imagery, and marks.
189. As a developer hand-writing Studio Script, I want plugin-contributed components and screens available with typed completion and diagnostics exactly as they appear in the Designer, so that visual and manual authoring stay one language.
190. As a designer, I want plugin, template, and station settings rendered automatically as organized tab groups of typed inputs, so that configuration surfaces stay consistent instead of each plugin inventing its own chrome.
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
115. As a first-time designer, I want a full-screen product welcome, so that I understand what Studio Designer is before creating an identity or opening a project.
116. As a returning designer, I want the product welcome dismissal remembered and available again from Help, so that onboarding does not obstruct routine startup but can be revisited.
117. As a designer, I want startup to discover identities already registered on this device, so that I can continue without entering an email before choosing the correct account.
118. As a designer, I want an account chooser that distinguishes Local Identities, Cloud Identities, ready sessions, signed-out sessions, and locked sessions, so that authentication state is clear.
119. As a designer, I want to choose whether a new identity is local or cloud, so that ownership and synchronization expectations are explicit before registration.
120. As a designer, I want to create a Local Identity with a display name, email, password, optional avatar, and remembered-session preference, so that local projects have a protected owner.
121. As a designer, I want image or video avatars for an identity, so that accounts are easy to distinguish on a shared device.
122. As a designer, I want to sign into a Local Identity entirely offline, so that local authentication does not depend on Studio Cloud.
123. As a designer, I want a locked or signed-out Local Identity to require its password, so that another person using the device cannot open its projects.
124. As a designer, I want a revocable “keep me signed in” session, so that trusted devices can resume quickly without storing my raw password.
125. As a designer, I want multiple Local and Cloud Identities on one device, so that personal and professional project ownership can remain separate.
126. As a designer, I want to sign out and return to the account chooser, so that changing identities does not require restarting the application.
127. As a designer, I want to sign into an existing Cloud Identity, so that I can access authorized synchronized projects and cloud-backed agent services.
128. As a designer, I want to create a Cloud Identity with a display name, email, and password, so that I can opt into Studio Cloud from the desktop application.
129. As a designer, I want cloud email verification, resend, and confirmation states, so that account ownership is established without leaving onboarding ambiguous.
130. As a designer, I want initial cloud setup to create or select my personal Studio workspace, so that synchronized project ownership has a clear cloud home without requiring an organization.
131. As a designer, I want cloud onboarding to confirm completion and take me to the Project Dashboard, so that account setup has a clear destination.
132. As a designer, I want cloud authentication failures, offline states, and expired sessions explained without hiding locally cached projects, so that connectivity problems remain recoverable.
133. As a designer, I want password and account recovery surfaces for both identity kinds where recovery is possible, so that loss of a session is not presented as loss of project data.
134. As a designer, I want account settings for profile, avatar, password, sessions, and sign-out, so that identity management does not require onboarding again.
135. As a designer, I want to inspect and revoke remembered sessions, so that access on a lost or retired device can be removed.
136. As a designer, I want provider credentials stored under the selected identity, so that agent models can be configured without placing secrets in a project.
137. As a designer, I want saved provider credentials to be named, masked, and revocable, so that I can manage access without a secret being displayed again.
138. As a designer, I want provider and model settings to show connection, compatibility, and availability, so that the agent model selector reflects actual usable models.
139. As an authenticated designer, I want the Project Dashboard to be my application home, so that projects are discoverable before entering the editor.
140. As a designer, I want the dashboard to show project name, ownership kind, synchronization state, conflict or recovery state, preview, updated time, and summary counts, so that I can choose a project confidently.
141. As a designer, I want a Recent collection ordered by meaningful activity, so that current work is fastest to resume.
142. As a designer, I want the last valid project optionally resumed after authentication, so that startup can return directly to ongoing work when I prefer it.
143. As a designer, I want dashboard search across project names and admitted metadata, so that a large project list remains manageable.
144. As a designer, I want filters for local, cloud, syncing, conflicted, recovered, and archived projects, so that project state can be isolated quickly.
145. As a designer, I want sorting by recent activity, name, creation time, update time, and project state, so that the dashboard can match the task at hand.
146. As a designer, I want a Grid dashboard view with visual project cards, so that project recognition can be thumbnail-led.
147. As a designer, I want an Index dashboard view with a compact sortable project list, so that dense project management is efficient.
148. As a designer, I want an Activity dashboard view combining projects with recent safe activity summaries, so that I can understand what changed before opening a project.
149. As a designer, I want Grid, Index, and Activity to be switchable dashboard view modes, so that the three Canvas-derived presentations are product features rather than development-only variants.
150. As a designer, I want dashboard mode, search, filters, sorting, and selection preserved while navigating and across sessions, so that changing presentation never resets my work.
151. As a designer, I want a clear empty dashboard state with Create Project, Import, and Templates actions, so that a new identity has an obvious starting point.
152. As a designer, I want to create either a device-owned local project or an authorized synchronized cloud project, so that project authority is chosen deliberately.
153. As a designer, I want new-project setup to capture a name, optional description, initial profile or template, storage choice, and cloud choice, so that a project starts with usable intent.
154. As a designer, I want dashboard actions to rename, duplicate, archive, restore, and safely delete a project, so that project lifecycle does not require filesystem manipulation.
155. As a designer, I want destructive project actions to explain local, cloud, asset, backup, and unsynchronized-operation consequences, so that deletion is informed and recoverable where promised.
156. As a designer, I want dashboard Import to admit a folder or selected files through Agent-led Ingestion, so that Canvas's import entry point is retained without creating unrestricted native format importers.
157. As a designer, I want an import review to show sources, provenance, inferred project content, warnings, and destination ownership before commands are applied, so that ingestion remains controlled.
158. As a designer, I want a Templates surface with first-party and admitted extension templates, so that common application structures can seed a Studio Project.
159. As a designer, I want templates searchable and filterable by application type, device profile, catalog requirements, and provider, so that the template library scales.
160. As a designer, I want a global application settings surface, so that preferences not owned by one project have one predictable home.
161. As a designer, I want theme, language, accessibility, motion, keyboard, startup, autosave, update, and diagnostic-sharing preferences, so that the native application respects my environment.
162. As a designer, I want project settings for metadata, storage, sync, Runtime identity, build/signing, extensions, capabilities, data, and recovery, so that project-level authority is inspectable outside the canvas.
163. As a designer, I want an Agents surface for provider/model configuration and enabled agents, so that the welcome selector and project agent behavior share one source of truth.
164. As a designer, I want a Skills and Tools surface for admitted agent and extension capabilities, so that available automation can be reviewed and revoked.
165. As a designer, I want New Conversation and Conversation History surfaces, so that Agent Conversations can be started, found, resumed, and safely removed outside the floating window.
166. As a designer, I want notification preferences for project, agent, sync, build, update, and security events, so that important background activity is visible without becoming noisy.
167. As a designer, I want Help to cover welcome, identities, projects, editor concepts, agents, extensions, preview, build, recovery, and keyboard commands, so that core workflows are discoverable in the application.
168. As a designer, I want a feedback surface that collects an explicit message and optional redacted diagnostics only with consent, so that reporting a problem does not leak project or credential data.
169. As a designer, I want About, license, dependency-notice, version, update-status, and release-channel surfaces, so that the native application is operationally complete.
170. As a designer, I want a synchronization conflict center reachable from the dashboard and project settings, so that unresolved work is visible even before opening the editor.
171. As a designer, I want migration and recovery surfaces for interrupted upgrades, failed restores, and quarantined projects, so that startup failures do not collapse into an unusable blank screen.
172. As a designer, I want consistent offline, connecting, synchronized, warning, and error indicators across authentication, dashboard, and project shell, so that network state is never inferred from missing content.
173. As a cloud user, I want plan, usage, and billing status available from account settings when paid cloud features exist, so that service limits and charges are transparent.
174. As a designer, I want every application-shell screen to support keyboard navigation, accessible names, focus restoration, reduced motion, zoom, and high-contrast behavior, so that accessibility begins before the editor opens.
175. As a designer, I want deep links and restart recovery to route through authentication and then restore the intended project destination safely, so that protected project links do not lose context.

## Implementation Decisions

### Product and ownership

- Studio Designer is a first-party desktop application implemented in Rust with a native GPUI interface. Desktop is the only Designer host in this specification.
- Studio Designer lives in the Studio Platform monorepo and extends the existing Runtime rather than maintaining a forked protocol or renderer.
- One Studio Project owns one Studio Design and produces one Studio Runtime Application.
- Studio Design, Studio Library, project configuration, extension declarations, revisions, and build metadata are project-owned. Runtime-owned user data is not part of Studio Design.
- Studio Designer replaces Studio Canvas's intended first-party authoring role. Instatic informs editor mechanics and interaction patterns but is not ported wholesale and is not a source-format dependency.

### Application shell and identity

- Introduce a first-party native application shell around the existing Designer workspace. It owns bootstrap, product welcome, identity selection and authentication, cloud onboarding, Project Dashboard, global settings, help/support, update/recovery routing, deep-link admission, and creation of a `DesignerSession` after a project opens.
- Product Welcome and Agent Welcome are different surfaces. Product Welcome appears before account selection on first launch and explains Studio Designer; Agent Welcome appears inside an opened project before an Agent Conversation begins.
- Startup follows an explicit state model: bootstrap local services; recover or migrate application data if required; show Product Welcome when not dismissed; discover identities; choose, create, or authenticate an identity; load Project Dashboard; then open a selected project. Protected deep links retain their intended destination through this sequence.
- A device may contain multiple Local Identity and Cloud Identity profiles. The account chooser displays safe profile metadata, identity kind, avatar, and session state without exposing project data before authentication.
- A Local Identity is password protected, device owned, and fully authenticates offline. It owns local projects and local preferences. Store a modern salted password verifier and revocable session metadata, never a recoverable raw password.
- A Cloud Identity authenticates through the Studio Cloud identity service. It can own synchronized projects and maintain authorized offline project caches, but Cloud authentication does not become a SurrealDB user login and never gives the desktop hosted database credentials.
- Local and Cloud Identity creation collect display name, email, password, optional avatar, and remembered-session preference. A local email is an identity label and does not require network verification; a Cloud Identity requires service-side email verification.
- Cloud onboarding provides register, verify/resend/confirm, personal workspace setup, completion, and recoverable failure states. The personal workspace is an ownership container for the same user and does not introduce organization collaboration in v1.
- Account recovery is authority-specific. Cloud recovery uses the Studio Cloud identity service. Local recovery can use only an explicitly configured recovery mechanism or a protected logical backup; the UI must not imply that Studio can recover an unknown local password otherwise.
- Remembered sessions are opaque, revocable, identity scoped, expire according to policy, and are stored through protected host facilities. Signing out clears the active session and returns to the account chooser without deleting projects.
- Avatar admission reuses Studio's safe media handling for image and video inputs, stores a deterministic local presentation variant, and never exposes the avatar source as project content unless separately imported.
- Account settings cover profile, avatar, password or recovery actions, remembered sessions, cloud state, provider credentials, plan/usage when applicable, and sign-out.
- Provider credentials belong to an authenticated identity, are stored through the operating-system credential facility or equivalently reviewed protected storage, are masked after entry, and can be listed by safe metadata and revoked. They never enter Studio Design, Agent Conversation content, diagnostics, project backups, or cloud sync unless the provider relationship is explicitly cloud managed.
- Global application settings cover theme, language, accessibility, reduced motion, keyboard, startup behavior, autosave defaults, notifications, diagnostic-sharing consent, update channel, and release information.
- Help, feedback, About, notices, and update/recovery surfaces are first-party shell destinations rather than placeholders. Feedback attaches redacted diagnostics only through explicit user consent.

### Project Dashboard and project lifecycle

- Project Dashboard is the post-authentication home and uses one host-owned project catalog query across local projects, synchronized cloud projects authorized to the selected identity, archived projects, recovery state, and safe recent activity.
- The dashboard has three first-class, user-switchable modes derived from Studio Canvas: **Grid** presents visual project cards, **Index** presents a compact sortable list, and **Activity** combines project access with recent safe activity. These are production preferences, not development concept variants.
- All three modes consume the same project catalog, search, filters, sort, selection, action availability, and status model. Switching mode cannot change project data or lose query state.
- Persist the selected dashboard mode and its non-sensitive query preferences per identity. Restore focus and selection when returning from a project where possible.
- Project search matches normalized name and admitted metadata. Filters include local, cloud, syncing, conflicted, recovered/quarantined, and archived. Sorting includes recent activity, name, creation, update, and state.
- Recent activity is derived from safe project, build, sync, recovery, and agent-history metadata. It must not disclose protected project content on the account chooser or before the owning identity authenticates.
- Every project summary includes stable identity, name, ownership/storage kind, sync state, last activity, safe preview, summary counts, build state where known, and actionable conflict/recovery indicators.
- Dashboard actions include Create Project, Import, Templates, Search, account access, global settings, help, feedback, and project lifecycle actions. An empty dashboard emphasizes Create Project, Import, and Templates.
- New Project chooses local or cloud authority, name, optional description, initial device/profile intent, optional template, and relevant storage/sync settings. Cloud creation requires a valid Cloud Identity and real service acknowledgement.
- Rename, duplicate, archive, restore, and delete use validated host operations with stable receipts. Delete explains local database state, cloud state, assets, backups, unsynchronized operations, and any recovery window before confirmation.
- Dashboard Import accepts selected files or a folder as a bounded one-time source for Agent-led Ingestion. It presents an import review containing source inventory, provenance, inferred Studio entities, warnings, destination identity/project authority, and the typed commands to be applied. It does not create dedicated native design-format importers or persistent arbitrary folder access.
- Templates include first-party templates and admitted extension templates. Template metadata declares application kind, supported profiles, required catalog kinds, Library content, extension dependencies, capabilities, version, and provenance. Instantiation creates ordinary Studio Design and Studio Library commands.
- Project Settings is accessible from the dashboard and opened project. It covers metadata, ownership/storage, sync, Runtime identity, build/signing, extensions, declared capabilities, data schema/migrations, recovery, and destructive lifecycle actions without duplicating editor inspectors.
- Agents, provider/model configuration, Skills and Tools, New Conversation, Conversation History, notification preferences, Help, feedback, account settings, About/notices, and updates are complete navigable shell surfaces.
- A conflict center and recovery center are reachable before opening a project. They show affected projects, safe status, required authentication or migration, recovery points, and explicit next actions.
- The shell uses consistent offline, connecting, synchronized, warning, and error states. Missing network data is never presented as an empty project list.
- The shell may offer optional resume-last-project behavior after successful authentication. Failure to reopen returns to the dashboard with a recovery diagnostic rather than trapping startup.
- Shared human workspaces, invitations, and multiplayer collaboration remain outside v1. The dashboard does not show nonfunctional “Shared with me” or invitation placeholders until that scope is deliberately added.

### Primary authoring seam

- Introduce a deep `DesignerSession` module as the primary interface used by the native shell, agents, MCP, tests, and build orchestration.
- A session owns the active project revision, selection, screen, device profile, tool, panel state, diagnostics, command registry, history cursor, agent runs, persistence coordination, and optional sync status.
- Callers submit typed queries and command batches and receive immutable snapshots, command receipts, progress, diagnostics, and conflict results. Callers never mutate stored entities directly.
- Focus View and Workbench View consume the same session snapshots and commands. View switching changes presentation only.
- GPUI types, SurrealDB types, cloud transport types, and Runtime `UiNode` types do not appear in the Studio Design interface.

### Studio Script authoring surface

- Studio Script is the canonical textual form of Studio Design. A project serializes as `.studio` source files using the rsvelte-flavored grammar: per-screen markup files plus a project file carrying tokens, responsive variants, interactions, plugin configuration, and settings. Every node carries its stable identity as an explicit attribute; markup references only catalog kinds, declared plugin SDK surfaces, tokens, and bounded binding paths.
- The Designer authors *in* this language rather than beside it. Adding a `ListView` on the canvas writes the corresponding element into the screen file; editing the file by hand updates the canvas after parse and validation. There are no parallel representations to drift apart: text is the canonical serialization of the model, derivable in both directions.
- All mutations, whether from canvas gestures, inspector edits, hand-typed characters, agents, MCP clients, or ingestion, flow through the same command engine operating on the parsed model. The text buffer is a representation, never a mutation substrate. Hand edits are parsed, validated, and diffed into typed commands or rejected with line-linked diagnostics.
- A deterministic canonical printer re-serializes affected files at batch-commit points. Identical model state yields identical bytes. Round-trip properties — `parse(print(model)) == model` and canonical formatting on republish — are tested invariants. Comments attach as trivia anchored to their following node and survive printing; other user formatting is normalized by design.
- Human-readable diffs for review, git history, and agent provenance are derived by printing before/after revisions; synchronization and receipts never carry text.
- Persistence keeps the embedded SurrealDB working store with its journal, revisions, and sync machinery, and adds lossless canonical Studio Script serialization so every project state is openable, diffable, and reviewable as text.

### Designer toolchain and language services

- Two grammars describe one language. A Tree-sitter grammar powers editor-latency work: syntax highlighting, outline, folding, and incremental reparsing while typing. The rsvelte parser of record performs full validation, feeds the command engine, drives the canonical printer, and fronts the compiler. Both grammars share one fixture corpus with CI equivalence tests so they cannot drift.
- A single diagnostics service produces all authoring diagnostics with stable safe codes, precise locations, and redacted context. The Designer problems panel, CLI, language server, and agent loop all consume the same feed.
- `studio check` validates projects and reports structured diagnostics; `studio fmt` enforces canonical formatting. Agents are required to verify batches with `studio check` and self-correct from structured failures before completing work.
- A lint policy layer adds severity-configurable warnings beyond validity, such as unused tokens, missing accessibility labels, deprecated bindings, and non-canonical formatting.
- A first-party language server speaks standard LSP and ships as an independent artifact that never requires a running Designer process. It layers Tree-sitter fast responses over parser-of-record semantics, providing completion for component kinds, token names, plugin SDK surfaces, and `$item` fields typed from declared response schemas, plus hover types and go-to-definition across project and plugin code.
- Standalone Studio IDE products remain outside this effort, but the embedded script editing inside Studio Designer uses these same services, keeping that path real without expanding current scope.

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

### Integration plugins

- First-party integrations such as GitHub sign-in or OpenAI-compatible AI endpoints ship as versioned **integration plugins**: self-describing packages that export their configuration schema, required secrets, OAuth provider behavior, REST/WebSocket route-group contributions, and the SDK surface applications compile against. A plugin describes everything it needs and everything it provides, and composes alongside other plugins rather than being wired into the host or the application individually.
- Plugin configuration is generated, not hand-built. The Designer renders an integration's setup surface directly from its declared configuration schema, the same way component inspectors derive from approved property schemas. A developer selects the plugin in project settings, completes its declared fields (client identifiers, scope choices, secret references, endpoint options), and the validated result merges into the project.
- At build time every enabled plugin contributes its capability, credential, and route-group declarations to the signed package. When multiple plugins are enabled, overlapping origin, route, or capability claims resolve through explicit merge rules; unresolvable overlaps become build diagnostics instead of silent overrides.
- Plugin runtime code ships with the paired SDK and compiles into `module.wasm`; host-mediated behaviors such as OAuth flows remain declarative descriptors executed by the Runtime host. Provider knowledge updates by shipping new plugin/descriptor versions, never by rebuilding authored applications. Third parties may author equivalent plugins under the extension authority rules above.
- Plugins contribute screens and Reusable Compositions that behave identically in both authoring modes: they appear on the Designer canvas and, as first-class library elements with typed completion and diagnostics, when hand-writing Studio Script. Visual and manual authoring consume the same contributions through the same language services.
- Plugins declare versioned **settings schemas**: named groups rendered automatically as top-level tabs containing nested typed fields — text, number, boolean, color, image, select, secret references, and device or station pickers. One generic renderer presents integration configuration, template options, and station-local settings alike. Plugins never ship custom settings chrome, and settings changes are ordinary tracked commands.
- Templates are plugins. Installing a vertical template applies ordinary tracked commands that instantiate its screens, compositions, token sets, settings groups, plugin references, and SDK imports into a project. Template brand surfaces map to tokens exposed as brand slots, so a customer rebrands an operational app by swapping token values rather than editing layouts.

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
- The compiler parses Studio Script as its single frontend. Both hand-authored projects and Designer-authored projects travel one path: canonical Studio Script source, validated parser-of-record output, then typed Studio intermediate representation with AssemblyScript/Wasm lowering behind the projection interface. Static structure serializes as projected tree data inside the package; declarative behaviors lower into guest logic. Compiler internals do not become the Studio Design source contract, and no textual export step exists between authoring and compilation.
- The output is a signed `.studio` package consumed by Studio Runtime. No HTML, website, mobile bundle, or third-party runtime export is required.
- Identical source revision, toolchain, declared build inputs, and signing configuration produce deterministic package contents except for explicitly isolated signature material.

### Runtime prerequisites and application capabilities

- Complete the generic semantic native renderer for every approved catalog kind before claiming full Designer coverage. Remove POS-specific identifiers and branches from generic rendering. Vertical surfaces removed from the generic path, such as point-of-sale catalogs, carts, receipts, print previews, and digital-menu components, return as first-party vertical component packages delivered through Reusable Compositions, extension contributions, or Studio Script SDK libraries rather than hard-coded renderer branches.
- Extend package and protocol contracts to carry the admitted Library snapshot, capability declarations, data schema/migrations, REST schemas, WebSocket schemas, and OAuth provider declarations required by the generated application. Payment use is declared through the same REST schema mechanism rather than a separate payment contract.
- Studio Runtime owns one logical embedded SurrealDB Runtime Data Store. Physical storage layout remains an implementation detail and need not be one literal file.
- Runtime derives an Application Data Namespace from verified publisher/application identity and prevents guest-selected namespace or database changes.
- Expose typed `data.collection` operations through generated SDK helpers. Applications may opt into bounded `data.surreal.query` only through a signed capability declaration.
- Bounded Surreal queries are parameterized, execute only inside the assigned Application Data Namespace, reject namespace/database switching, and cannot reach Studio system data, filesystem, database scripting, or database-initiated networking.
- Signed application data migrations execute through a host-owned pre-launch lifecycle with backup, version checks, idempotency, rollback/recovery policy, and separate authority from normal data operations.
- REST access is a host-owned request broker. Signed declarations constrain origins, paths, methods, headers, request and response schemas, sizes, redirects, timeouts, retries, rates, and redaction. Default limits are generous for real application workloads while remaining explicit, declared, and auditable.
- WebSocket access is a host-owned session broker. Signed declarations constrain endpoint, subprotocol, authentication reference, outbound/inbound message schemas, sizes, rates, reconnect behavior, lifetime, and lifecycle events, with generous explicit limits. Guests receive session identities and typed events, never sockets.
- OAuth is host-owned and provider-plugin based, modeled on maintained social-login frameworks such as Better Auth. Each supported provider is a first-party, versioned, declarative integration descriptor covering endpoints, scopes, PKCE or confidential-client behavior, profile mapping, refresh semantics, and known provider quirks such as missing refresh tokens or private email handling. A package enables providers by declaration and supplies its client ID and secret through protected configuration; the Runtime host executes the browser/redirect or device flow, stores refresh/access tokens in protected host storage, and exposes approved identity claims, status, and provider action results to the application. Provider descriptors ship with the paired Runtime SDK so the core runtime path stays small, descriptors update independently of authored applications, and unknown, outdated, or revoked providers fail safely with diagnostics instead of falling back to generic network access.
- Payment providers are not bespoke host capabilities. Applications call official Stripe test-mode APIs through the schema-constrained REST broker using restricted test keys supplied as protected configuration. Host mediation still enforces transport bounds, redacts key-shaped values from diagnostics and logs, and provides protected confirmation surfaces where a payment flow requires one.
- The REST broker supports declared streaming responses using server-sent events: typed incremental chunk events validated against the declared response schema, explicit size/rate/duration bounds, guest-initiated cancellation, and lifecycle events. Reconnect and retry policy belongs to the host, not the guest.
- REST route groups declare their credential source explicitly: an OAuth provider plugin session, a named secret reference, or public access. The broker injects credentials at send time from protected storage; plaintext never enters guest memory.
- Application secrets are declared in the package by name and purpose without values. Publisher-configured secrets (OAuth client secrets, provider keys) are supplied out-of-band through protected release or install-time configuration; user-supplied secrets are captured through host-owned entry surfaces with declared prompt text.
- A single host-owned protected secret store partitioned per verified application identity backs all credential material through the operating-system credential facility. Guests can query only status metadata such as configured, missing, or revoked; no read path for secret values exists in the guest interface. Rotation replaces values without rebuilding or re-signing packages. Secret values never enter Studio Design, SurrealDB records, sync payloads, backups, diagnostics, or history records.
- Capability admission extends the existing principal, trust, action-gate, redaction, Wasm budget, and no-WASI model. Denied or unavailable capabilities fail safely with source-linked diagnostics.

### Application users, roles, and shared operation

- Generated applications support their own application-level users, distinct from OAuth provider identities. Credential kinds include employee PIN or badge entry and email/password sign-in, validated against the application's declared user store inside its data namespace.
- Roles bind to routes, screens, actions, and individual data records. Row-scoped access — a technician seeing only assigned tickets, a moderator only assigned channels — is enforced by the Runtime host at data access, not by interfaces hiding options. Role assignment changes are auditable operations.
- Applications declare a **data topology** at build time: device-local namespace for single-device apps, or a center server holding shared operational truth for multi-station apps. The center protocol is identical whether deployed as a Studio Cloud-hosted namespace or as a self-hosted on-premises hub; choosing between them is deployment configuration, invisible to authored code.
- Stations in a shared topology hold only local typed settings — station identity, center address, cache policy — contributed through plugin settings schemas. Operational reads and writes resolve through the SDK against the center; during disconnection, stations queue writes locally and replay with explicit conflict preservation when connectivity returns, reusing the same outbox, journal, and conflict machinery specified for Designer synchronization.
- Inbound webhooks are host-owned listeners admitted through signed declarations that constrain endpoint identity, payload schemas, source verification, sizes, rates, and lifetime. Validated events route into the application like any other typed input; no guest code ever binds ports.
- Scheduled and event-triggered workflows are declarative: time, interval, and event triggers execute bounded typed actions against app state and plugins, running wherever the declared data topology keeps authoritative state. Workflow definitions participate in validation, diagnostics, and audit exactly like interactions.
- Development, staging, and production environments isolate application data and secret values per environment. Packages reference secret names; environment configuration supplies values, so promotion between environments never moves credentials.
- An append-only audit log records security-relevant events: authentication attempts, role and user changes, destructive actions, exports, webhook admissions, and workflow runs. It is queryable in the Designer, exportable by its owner, and redacted under the same rules as all diagnostics.
- Deployed applications receive updates through a signed update channel with staged rollout, health checks, and rollback. Updating many stations is one channel operation, never a per-machine task.

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
- Runtime acceptance adds real embedded application data, a schema-constrained REST request, a host-owned WebSocket session, an OAuth sign-in through an integration plugin configured inside the Designer and exercised against the official provider sandbox, and a Stripe test-mode payment performed through the REST broker. A small OAuth proof application, such as a GitHub repository viewer, precedes the POS journey to certify the provider-plugin flow end to end before the full reference application depends on it.
- The POS journey remains the first complete authoring-to-Runtime proof. A multi-terminal **restaurant operations** journey is the target flagship that extends it with application users and roles, shared center-server table/check state, station offline recovery, kitchen displays, hourly time tracking feeding payroll exports, split and single billing flows, and real receipt and kitchen printing. Streamer multistream moderation and rental-property management (tenant portal, technician ticket scoping, rent payment) are recorded as additional first-party vertical template candidates rather than v1 commitments.
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

- Launch the real GPUI Studio Designer from a clean application-data directory and exercise Product Welcome, identity flows, Project Dashboard, supporting shell surfaces, and the opened project in both Focus and Workbench views.
- Verify first launch, welcome dismissal/revisit, identity discovery, account chooser states, Local Identity creation/login/unlock/sign-out, remembered-session expiry/revocation, multiple identities, avatar admission, startup recovery, and protected deep-link restoration through visible shell behavior.
- Verify Cloud Identity login, registration, email verification/resend/confirmation, personal workspace setup, completion, expired-session recovery, offline cached-project behavior, and sign-out against the real Studio Cloud test identity service.
- Verify Project Dashboard empty and populated states, recent activity, search, filters, sorting, project status, safe previews, Create Project, Import review, Templates, project lifecycle actions, and optional resume-last-project behavior.
- Verify Grid, Index, and Activity as interchangeable dashboard modes over the same catalog and query state, including preference persistence, keyboard focus restoration, and identical action availability.
- Verify account, provider/model, global application, project, notification, Help, feedback, About/notices, update, conflict, migration, and recovery surfaces through their user-visible outcomes rather than route existence.
- Verify that view switching preserves session state and that every persistent Workbench surface is reachable from Focus View.
- Exercise mouse, keyboard, focus traversal, command bar, hierarchy editing, drag/drop, resizing, guides, inspector edits, responsive profiles, prototype interactions, diagnostics, history, live agent progress, cancellation, undo, preview, and build readiness.
- Verify the clean agent welcome surface, company-mark/model-control placement, searchable model selection, minimal composer, Import flow, absence of terminal/full-access controls, first-message transition, floating conversation movement/resizing/collapse, and preservation across Focus and Workbench views.
- Verify every Agent Reference target kind through mouse and keyboard activation, including correct selection/panel navigation, current-label resolution after rename, historical revision context, and safe stale, missing, or denied states.
- Accessibility acceptance includes meaningful roles and names, complete keyboard operation, predictable focus, focus restoration, target sizes, contrast, reduced-motion equivalence, screen-reader-visible validation, and profile-specific input semantics.
- Visual evidence may use deterministic screenshot comparisons for stable overlays, frames, and selection chrome, but semantic and interaction assertions remain authoritative.

### Authoring-to-Runtime seam

- Starting from the canonical POS Studio Project, use the public Designer build command to validate, project, package, sign, launch, and exercise the real Runtime Application.
- Assert multi-screen navigation, state, collections, forms, overlays, component events, accessibility, responsive variants, offline Library content, persisted Runtime data, signed migrations, failure recovery, and source-linked diagnostics.
- Execute Stripe test-mode payments through the REST broker against official Stripe test APIs, OAuth sign-in through an enabled first-party provider plugin against the official provider sandbox/test application, REST against the approved real test endpoint, WebSocket against the approved real test endpoint, and Runtime data against real embedded SurrealDB.
- Credentials are supplied through protected test/release configuration and never committed, printed, snapshotted, placed in guest memory, or persisted in project artifacts.
- Live integration suites may be credential-gated during ordinary local development, but release acceptance cannot pass without running them successfully. Simulators do not substitute for this gate.

### Existing supporting seams

- Extend protocol fixture and compatibility tests for every new closed schema, unknown-field rejection, limits, stable error codes, version negotiation, and malicious payload.
- Extend retained registry tests for atomic mounts/patches, source identity preservation, targeted updates, parent relationships, limits, and rollback.
- Extend component catalog tests so every declared kind proves schema, semantic native mapping, editor metadata, interaction behavior, accessibility, and no generic fallback.
- Extend package tests for deterministic contents, Library snapshots, capability declarations, data migrations, path traversal resistance, signature verification, tamper detection, and size/count limits.
- Extend Wasm and security tests for no-WASI isolation, budgets, capability denial, opaque handles, secret lifecycle, redaction, REST/WebSocket admission, OAuth/payment mediation, and safe termination.
- Extend Designer application-shell integration tests for local password verification, session revocation, identity isolation, protected credential storage, cloud-token handling, dashboard catalog admission, deep-link authorization, redacted feedback, update/recovery routing, compositor loss, and shutdown cleanup.
- Extend Runtime application-shell integration tests for Runtime Data Store isolation, migrations, network brokers, protected confirmation, navigation recovery, compositor loss, and shutdown cleanup.
- Extend language and toolchain tests with round-trip invariants — `parse(print(model)) == model`, canonical formatting on republish, comments preserved as anchored trivia — plus dual-grammar corpus equivalence between the Tree-sitter grammar and the parser of record.
- Extend application-operation tests for role enforcement including row-scope bypass attempts through direct data access, station disconnect queueing and replay with conflict preservation, webhook payload validation against malicious inputs, workflow scheduling determinism, environment isolation of data and secrets, audit completeness under every recorded event class, and update-channel signature verification, staged rollout, and rollback.

### Required non-functional evidence

- Component coverage is release-blocking until every approved kind reaches semantic native rendering, Designer editability, Runtime verification, and applicable host certification.
- Determinism is release-blocking: repeated projection and package builds from identical admitted inputs must match at the declared deterministic layers.
- Durability is release-blocking: no accepted command may disappear after the documented durability point, and crash recovery must return the last durable revision without a partially committed batch.
- Sync correctness is release-blocking: every accepted operation is applied exactly once logically, conflicts preserve both intents, and no tested disconnect/retry sequence loses acknowledged or unsent work.
- Security is release-blocking: capability bypass, cross-application data access, raw credential/token/socket/database-handle exposure, unredacted persistence, unsafe SVG, path traversal, invalid signature admission, and extension escape tests must all fail closed.
- Accessibility is release-blocking for the native Designer and canonical Runtime journey, with remaining human assistive-technology review recorded as required release evidence.
- Application-shell completeness is release-blocking: first-run, both identity types, all three dashboard modes, project creation/import/templates, settings/support, offline/error, conflict, and recovery paths must have functional and accessibility evidence; a navigable placeholder does not satisfy coverage.
- Performance benchmarks cover clean build, binary/package size, cold launch, project open, common command latency, canvas interaction, projection, incremental preview, full build, 10,000-operation replay, sync catch-up, memory use, and embedded-store compaction on the supported baseline machine.
- Before release, the project records baseline hardware and explicit budgets for those benchmarks; continuous integration fails regressions beyond the approved tolerance. A benchmark without an approved budget is evidence collection, not certification.
- Recovery rehearsal covers logical backup/restore, Studio schema migration, SurrealDB patch upgrade, deliberately incompatible engine fixture, failed extension migration, interrupted asset transfer, and application data migration failure.
- Run the repository's locked workspace tests, strict Clippy, formatting check, release build, fuzz targets, dependency audit, SBOM generation, license review, and signed-package verification as release gates.

## Out of Scope

- Web or mobile Studio Designer hosts in this effort.
- A standalone Studio IDE product in this effort. The language server, both grammars, `studio check`/`studio fmt`, and embedded script editing inside Studio Designer reserve that path without building it.
- Runtime execution certification on hosts that do not yet exist, while retaining portable contracts and packaged variants for future hosts.
- Export to HTML, CSS, React, Svelte, Flutter, mobile application bundles, websites, Figma, or any runtime other than Studio Runtime.
- Dedicated Rust importers for HTML, design tools, images-as-layout, or other external design formats; agents perform Agent-led Ingestion.
- Treating HTML, `UiNode`, Wasm, AssemblyScript, generated code, or the projected Runtime tree as the editable source of truth.
- Arbitrary JavaScript, database scripting, HTML/CSS injection, unrestricted native plugins, raw GPUI access, arbitrary rendering, or third-party native component implementations.
- Direct guest, agent, MCP, or extension access to SurrealDB, provider credentials, OAuth tokens, sockets, filesystems, or cloud database credentials.
- Terminal switching, unrestricted “full access” agent modes, arbitrary folder mounting, or a general-purpose coding-agent console inside Studio Designer.
- General-purpose customer database administration or a database-as-a-service product.
- Simultaneous multi-user collaboration, invitations, organizations, presence, comments, multiplayer cursors, or human-to-human real-time coediting in v1.
- Canvas editor, HTML artifact, proposal-review, iteration/diff, responsive web shell, public preview/share, client-review, and administrator pages already superseded by the Studio Design editor or excluded collaboration scope.
- Public website CMS, public content publishing, or making Designer the live production database for generated applications.
- Production payment processing certification, production OAuth-provider approval, merchant onboarding, legal/compliance approval, and production cloud operations beyond the official sandbox acceptance required here.
- Automatic generation of arbitrary application business logic beyond the declared Studio interaction, data, extension, and Runtime capability contracts.
- SurrealKV as the shipping Designer persistence backend unless a later qualification decision replaces RocksDB.

## Further Notes

- Runtime prerequisite work is intentionally part of this specification. It may and often should land before the Designer feature that consumes it.
- The existing protocol, retained UI registry, component mapping, native state/event handling, security principals and capabilities, Wasm sandbox, signed package format, native shell, navigation, and POS journey are the reusable foundation. Their current existence is not evidence that the Designer-specific or generic renderer requirements are complete.
- Studio Canvas's first-run welcome, account chooser, local/cloud identity setup, password unlock, cloud onboarding stages, Project Dashboard, Grid/Index/Activity concepts, project actions, provider credential management, settings, templates, notifications, Help, and feedback are behavioral references for the new native shell. Their Svelte/Tauri implementation and editor/artifact routes are not production dependencies.
- The three confirmed acceptance seams are `DesignerSession`, the native Designer shell, and the complete authoring-to-Runtime journey. Supporting module tests provide diagnosis and contract confidence but cannot replace those seams.
- The accepted editor prototype is a primary UX reference under the Studio Designer Wayfinder artifacts. Its code is throwaway and must not be promoted into production architecture.
- The supplied agent-interface references establish the visual direction for the clean welcome composer, searchable model picker, floating post-message conversation, and icon-bearing inline Agent References. Studio adopts those interaction ideas without inheriting coding-agent terminal or unrestricted-access controls.
- Exact cloud hosting provider, OAuth sandbox provider, REST test service, WebSocket test service, baseline performance hardware, and release budgets must be recorded in the implementation tickets or release configuration before their dependent acceptance ticket can pass.
- This specification is intentionally broad because it defines the complete v1 journey. The next step is `/to-tickets`, which must preserve end-to-end tracer bullets and explicit blocking edges rather than translating each heading into a horizontal implementation ticket.
