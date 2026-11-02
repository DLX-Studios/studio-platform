# Studio project completion audit — 2026-11-01

Source baseline: `e21ed734`. Scope: Runtime/toolchain specs 001–014, Designer decisions and
tickets 01–65, the shipped Designer/Runtime entrypoints, and release evidence. This is a
source-and-existing-evidence audit; the native acceptance journey was traced, not executed
through a graphical desktop. No application behavior was changed during this audit.

## Outcome

Studio has substantial tested domain, compiler, persistence, sandbox, networking, and packaging
code. Its shipped Designer does not yet connect those pieces into the complete approved
create → edit → preview → signed package → Runtime journey. Several native surfaces remain
summaries or adapters, and some completed tickets overstate their user-visible implementation.
The first work is project ownership/isolation, followed by native preview parity and the
Designer build/launch bridge. A percentage-complete estimate would conceal those gaps.

The executable journey and ordered work are maintained in:

- [Designer acceptance journey](DESIGNER_ACCEPTANCE_JOURNEY.md)
- [Completion backlog](../../.scratch/studio-designer/completion-tasks.md)
- [Current roadmap](../ROADMAP.md)

## Evidence rules

- A model or broker test proves that layer, not a shipped widget, provider exchange, or installer.
- A source-string assertion proves source wiring text is present, not that a user completed a UI flow.
- A native socket/CLI CI job does not certify the full GPUI application on that operating system.
- A missing connection below means no connection was found in the named shipped call paths;
  existing lower-level implementations should be reused, not rewritten from scratch.
- The previous local checkpoint passed formatting, workspace Clippy with warnings denied, and
  1,290 workspace tests (2 intentionally ignored). That evidence is reused here; it does not
  erase missing acceptance coverage. Latest native Windows/macOS reload and CLI CI jobs passed
  for `e21ed734`. The Linux validation job subsequently failed with runner disk exhaustion
  while its Rust test step was in progress; see F17.

## Findings

| ID | Severity / kind | Requirement | Source evidence and remaining work |
| --- | --- | --- | --- |
| F01 | Critical / partial | Constitution I; tickets 54, 55 | `NativeProductShell::new` resets `next_project_number` to 1; `create_project` generates `local-project-N`; `open_or_create_focus` shares one store; `designer_persistence::project_batch_id` uses only the project ID. Separate identity catalogs can reference the same durable project key. `NativeProductState::open_project` checks authentication, not ownership, and sign-out does not clear the retained Focus entity. Source confirms the collision path; a native two-identity reproduction is still required. Add ownership-scoped open/persistence, restart-safe IDs, and session disposal. |
| F02 | High / contradicts | Designer decisions 04, 10; ticket 39 | `FocusView::render_node` in `crates/studio-designer/src/focus_view.rs` recursively draws labeled `div` boxes. It does not use Runtime controls or render authored text/style/event semantics. `studio-app/src/foundation.rs` has the separate Runtime renderer. The audited canvas has selection click handlers and action buttons, without native drag/resize/hierarchy-drag wiring. A valid `UiNode` projection is not native preview parity. |
| F03 | High / missing | Designer decision 10; v1 baseline 04 | The Designer entrypoints and Focus toolbar have no call to package/sign/launch a Design revision. CLI builds and `studio-package::pack_bundle` exist, but there is no audited native Designer → pinned Design/Library snapshot → production backend → verified Runtime launch connection. Define and implement that bridge and a user-visible Build/Run flow. |
| F04 | Resolved shipped-path integration; remaining platform gates | spec 004 FR-004, FR-011 | Follow-up: `studio-cli::dev` owns the native window, interprets Studio IR, commits compatible live state and swaps cached loose Wasm artifacts with SDK state hooks. Seven native Wayland revisions preserved one window without creating packages. Shared host code is in `studio-runtime`; production `studio-app` has no dev launch selector. Current compiler unit and platform limits are recorded in `HOT_RELOAD.md` and the feature 004 validation follow-up. |
| F05 | High / partial | tickets 48, 49; Designer decisions 04, 06 | `open_or_create_focus` passes `None` as the Library snapshot. The Library button changes workspace state, and the Workbench Library panel is a label. `library_assets::generate_runtime_variant` copies original bytes rather than decoding/transcoding. Admission, content models, and adapters exist; native import/bind/content flows and a truthful decoder/variant matrix remain. |
| F06 | High / partial | tickets 50–52; decision 08 | `studio-designer/src/mcp_surface.rs::apply_batch` returns `Ok(())` without a batch/session and returns fixed attribution/diagnostic strings. `agent_adapter.rs::cancel` is a no-op, and scoped reads just format an event. Real `AgentChannel`, scoped access, and `McpClient` exist in `studio-design`; connect them to host transports, the real checker, and native conversation/progress UI. |
| F07 | High / contradicts | decision 08; ticket 50 override | `agent_channel.rs` tests require stale overlapping edits to return a conflict and two agent batches to share one undo entry. The accepted override requires last-writer-wins at batch granularity and independent per-batch undo. Reconcile implementation and tests with that explicit decision; preserve the separate cloud-conflict policy. |
| F08 | High / partial | tickets 45, 49, 51, 53 | `FocusView::render` builds Workbench panels with `panel_label` summaries. ContentEditor, ConversationComposer, and PluginTemplateUx have useful tested APIs but no call sites in the shipped Focus/bootstrap composition. Wire interactive Library/content/plugin/conversation/history panels and real docking/input behavior. |
| F09 | High / partial | tickets 54–58 | Native local identity inputs exist, so ticket 54's old “stub shell” note is stale. However Settings visibly exposes reduced motion and summaries; Help/About/sync/conflict/recovery routes fall through to generic navigation buttons. Model/controller tests do not complete these native screens. |
| F10 | High / partial | tickets 31–34; v1 every-kind rule | `studio-components/tests/catalog_mapping.rs::renderer_certification_reports_only_the_documented_time_picker_gap` deliberately expects certification to fail for TimePicker. Implement its semantic renderer, inspector/events, and parity tests; then run the full catalog certification and native accessibility checks. |
| F11 | High / partial | tickets 15, 16, 19, 20, 26, 27; v1 baseline 04 | The audited `PluginSurface::queue_action` routes actions to `run_provider_action`, which admits provider OAuth `sign_in`; other capabilities are denied there. The data, query, WebSocket, webhook, and workflow services have host APIs/tests, but their presence does not prove a guest-callable production integration. Add explicit closed dispatch/SDK/event wiring and real guest tests for each admitted capability. |
| F12 | High / partial | decisions 04, 07; ticket 35 | Registry descriptors contain compositions/settings/commands/actions/templates and a simulator-only declared capability enum. The approved v1 extension scope also names content types, validators, and migrations. `lifecycle.rs::HookRunner` checks elapsed time after callback return and bounds output bytes, not live guest memory/preemption. Specify missing contributions, connect sandbox interruption/resource enforcement, and wire consent/install/settings/removal UI. |
| F13 | High / missing integration | decision 09; tickets 57, 58 | Bootstrap constructs `SyncCoordinator<OfflineSyncWorker>` and always reports transfer unavailable. Cloud onboarding/conflict/recovery state machines exist, but no provisioned identity/sync/asset-transfer service is connected to the product shell. Supply the service contract/configuration and wire resumable sync and recovery UI. |
| F14 | High / verification gap | tickets 19–21, 34, 36, 59, 64 | Real-endpoint network gates are feature-gated; provider live setup requires operator credentials. The flagship report explicitly keeps physical hardware, peripheral, live Stripe, and accessibility gaps blocking. Deterministic fixtures remain useful but do not close those gates. |
| F15 | High / verification gap | constitution V; quality decision 12 | Full application builds/native UI runs on Windows/macOS and Linux Wayland/X11, installers/signing, update deployment rehearsal, current dependency/license review, accessibility and size measurements need release evidence. Socket/CLI CI is a narrower gate. Formal low-power hardware certification remains an explicit deferral, not a pass. |
| F16 | Medium / tracking contradiction | roadmap, specs, tracker | At audit entry specs 005–008 had seven unchecked tasks despite roadmap completion claims; 009–014 were marked Draft although implementation paths exist. Several Designer tickets say ready-for-agent or code-only despite tested seams. The audit closes two documentation tasks, leaving five existing verification tasks open, and qualifies the Draft statuses. Do not check native/live gates based on model tests. |
| F17 | High / verification gap | Integration/release gates; AGENTS verification policy | Latest Linux CI validation job failed before Rust tests completed. The runner annotation reports `System.IO.IOException: No space left on device` while writing its worker log. Windows/macOS jobs succeeded. Local 1,290-test evidence remains valid, but the full remote workflow is not green. Fix CI artifact/profile/disk budgeting and obtain a fresh completed run; do not blindly rerun the same oversized configuration. |

F01 is a source-confirmed ownership design problem, not a claim of an executed security exploit.
Its regression/reproduction is the first backlog task. F06 does not establish a privileged MCP
bypass: the current surface does not perform a mutation at all; it incorrectly acknowledges work.

## Evidence inventory

| Area / existing tickets | Present evidence | Status after reconciliation |
| --- | --- | --- |
| LocalStore, application data, migrations (14–18) | `studio-host/tests/local_store_*`, `application_data`, `bounded_surreal_query`, real crash/migration tests; protected-secret tests | Implemented/tested foundations; production callers and ownership remain independently assessed. Typed collection rows and bounded-query tables are intentionally disjoint per ticket 16. |
| Network, OAuth, provider admission (19–21, 36, 60–65) | `studio-net` broker suites; `studio-oauth` flow/broker suites; `tests/integration/plugin_launch.rs` | OAuth host wiring tested with deterministic providers; live provider/network evidence open; general guest capability dispatch remains F11. |
| RBAC, topology, webhooks, workflows, audit, updates (24–30) | Host/net integration tests and `studio-update-channel/tests/update_channel.rs` | Tested host services; do not claim universal native/guest/service deployment integration. |
| Renderer catalog (31–34) | Catalog readiness tests and Runtime foundation renderer | TimePicker fails certification by design; native preview also has F02. |
| Registry (35) | `tests/integration/pos_pack_registry.rs` | Signed descriptor/consent/lifecycle seam tested; expanded contribution and native UX gaps F08/F12. |
| Design/session/editing (37–47) | `studio-design/tests/designer_session_seam.rs`, manipulation/layout/tokens/responsive/prototype/workspace/script tests; Designer Focus tests | Models and editing actions exist. Actual native rendering, docking completeness, full profile UX, and build output are not thereby certified. |
| Library/content/agent/MCP/plugins (48–53) | Host Library tests; Design content/agent/MCP/template tests; `studio-designer/tests/sprint04_surfaces.rs` | Implemented lower layers with explicit adapter, semantic, and native composition gaps F05–F08/F12. |
| Identity/dashboard/settings/sync/recovery (54–58) | Native bootstrap, host identity, dashboard/settings/controller and conflict/recovery tests | Local inputs and persisted catalog exist; F01 and product-screen/cloud integration remain. |
| Flagship (59) | `studio-flagship` deterministic report and tests | Explicitly not release-ready; external gates remain blocking. |
| CLI/compiler/editor specs 009–014 | Interaction/shape/structural compiler tests, `studio-cli/tests/new_project.rs`, VS Code template tests | Code exists; Draft status is not proof of absence. Reconcile against acceptance scenarios, including genuine editor smoke tests. |

## Proposed additions versus the approved baseline

The native sidecar bridge and Rust/Go/C Media Pipeline Lab are a saved proposal with no
implementation yet. They are tracked as planned additional work, not silently substituted for
the sandboxed declarative extension contract. Web/mobile Designer hosts remain outside the
existing Designer v1 scope; broader Runtime/browser/mobile delivery needs explicit platform
work and acceptance evidence. The earlier research's theme-file/named-breakpoint API should be
compared with the implemented Design token/responsive models before promising that syntax.

## Latest CI evidence

Run: [41280736076](https://github.com/DLX-Studios/studio-platform/actions/runs/41280736076),
commit `e21ed734`. Both native reload/CLI jobs succeeded. Linux validation passed formatting and
Clippy, then the runner failed during `Test Rust` with disk exhaustion. Its check annotation
is attached to [job 124771984623](https://github.com/DLX-Studios/studio-platform/actions/runs/41280736076/job/124771984623).
The failed runner log could not be retrieved; the annotation and incomplete step status are the
available evidence. No completed remote Rust test result or downstream release/smoke result is claimed.
C021 restores that verification path; this audit does not modify the workflow or rerun it.

## First delivery slice

Start with [ticket 66](../../.scratch/studio-designer/issues/66-project-ownership-isolation.md),
then [67](../../.scratch/studio-designer/issues/67-native-preview-runtime-parity.md),
[68](../../.scratch/studio-designer/issues/68-design-build-sign-launch.md), and
[69](../../.scratch/studio-designer/issues/69-designer-acceptance-journey.md).
The acceptance run is currently blocked by implementation gaps; this audit does not mark it passed.

## Follow-up implementation evidence — 2026-11-01

F01 is addressed by [ticket 66](../../.scratch/studio-designer/issues/66-project-ownership-isolation.md)
and the [ownership contract](DESIGNER_PROJECT_OWNERSHIP.md): 51 Designer tests and three host
persistence tests pass, including production authentication, foreign-ID denial and child-process
cold reopen. Native GUI acceptance remains separately tracked. The historical findings above
remain the baseline audit; the completion backlog records current implementation progress.

F17 is closed by [run 41285190138](https://github.com/DLX-Studios/studio-platform/actions/runs/41285190138)
on `069eff9b`: Linux validation and macOS/Windows transport jobs all succeeded. Verification
profiles now omit debug symbols/incremental artifacts, checks remain serialized, and startup uses
the already-built debug probe. The preceding run's Rust checks passed with a 12 GB target directory
and 67 GB runner disk remaining, then exposed a stale Wayland diagnostic assertion; that assertion
was corrected before the successful run. This is CI evidence, not full cross-platform release certification.


## Native preview and Library follow-up — 2026-11-01

C002/C003 now use a shared Runtime-native renderer with seven production-view tests covering
selection, saved edits, prototype isolation, native TimePicker, profile sizing and token layout.
C007 has project/session-scoped Library access and serialized concurrent catalog updates.
Unsupported resize/quality/conversion requests now fail instead of publishing unchanged bytes
as transformed output; [the capability matrix](LIBRARY_CAPABILITIES.md) records decoder limits.
Native Library authoring and preview-byte loading remain open, along with signed Build/Run and
the other completion backlog items. No whole-project completion or platform release is claimed.

Clean-cache integration evidence: format check, 1,307 workspace tests (two qualifications
ignored), warnings-denied workspace Clippy, and 39 Bun tests including headless Wayland pass.
