# Studio completion backlog — 2026-11-01

Source: [completion audit](../../docs/development/PROJECT_COMPLETION_AUDIT.md), baseline
`e21ed734`. This is the ordered implementation handoff, not a statement that these changes
have been implemented. Existing accepted requirements remain authoritative. Start each behavior
change with a failing test, then use focused checks; use the shared Cargo cache sequentially.

Progress: C001 is implemented with focused host/lifecycle evidence. C004 is specified in the
[build contract](../../docs/development/DESIGN_BUILD_CONTRACT.md); C005/C006 remain unimplemented.
C021 is complete: [run 41285190138](https://github.com/DLX-Studios/studio-platform/actions/runs/41285190138) passed Linux validation and macOS/Windows transport jobs. C009 is implemented at
the shared domain boundary; native transport wiring remains C010. C002 now shares the native renderer with focused interaction/geometry evidence; native token/profile cases now pass; asset and signed-launch acceptance remains. C003 has native TimePicker/inspector implementation and passing catalog certification; OS acceptance remains C019. C007 now has project/session-scoped host Library access; native Library integration remains open. All other tasks remain open.

Integration checkpoint after a clean shared-cache rebuild: format check, 1,302 workspace tests
(two explicit qualifications ignored), workspace Clippy with warnings denied, and 39 Bun tests
including real headless Wayland startup pass. The earlier ownership/live-editing commit also
passed [remote CI](https://github.com/DLX-Studios/studio-platform/actions/runs/41286226664).
The new native preview changes require their own remote run.

## Delivery order

Complete C001 first. C002/C003 establish real native rendering. C004 defines the missing build
contract before C005/C006 implement it. C007–C014 complete product capabilities. C015 is the
single local acceptance gate. C016–C019 close verification/release gates. C020 is the separately
proposed native integration feature. C021 repairs the observed remote CI disk failure and should
run alongside the first slice before accepting later integration checkpoints. Independent work may proceed only after its dependencies.

| Task | Priority | Work / acceptance | Depends on | Requirement / finding |
| --- | --- | --- | --- | --- |
| C001 | Critical | **Implemented; see ticket 66 evidence.** Reproduce two identities creating/opening `local-project-1`, restart collisions, direct foreign-ID open, and retained Focus after sign-out. Enforce host-owned ownership and safe identity allocation; clear session surfaces. No cross-owner read or mutation; same-owner reopen remains durable. See ticket 66. | — | 54/55, Constitution I; F01 |
| C002 | High | Replace labeled-node preview with shared Runtime-native rendering while preserving author selection overlays and stable IDs. Authored Text, Input, Button, styles/layout and events must match the launched app. See 67. | C001 | 04/10/39; F02 |
| C003 | High | **Implemented locally; native UI and catalog tests pass.** Implement semantic TimePicker rendering, inspector/event/property contract and keyboard/accessibility tests; certify the entire approved catalog with zero fallback. | C002 | 31–34; F10 |
| C004 | High | **Specified; see build contract.** Specify a closed build request for an immutable Design revision + Library snapshot + target/manifest + host signing reference. Define diagnostic mapping, reproducible inputs, failure/last-good behavior, and shared projection semantics. | C001 | 04/10; F03 |
| C005 | High | Implement Design-to-existing compiler/package backend; preserve node/events/navigation, responsive variants and referenced asset keys. Two builds of the same inputs must be byte-identical and signature/asset tampering must fail Runtime admission. | C004 | 10; F03 |
| C006 | High | Add native Build/Run commands using C005, trusted signing configuration and verified Runtime admission. Show structured failures, preserve last-good artifact, and reopen the same durable source. See 68. | C002,C005 | 05/10; F03 |
| C007 | High | Connect the real host Library and immutable projection snapshot to native import/browse/insert/bind/delete UI and content fixtures/forms. Publish a tested decoder/variant capability matrix; do not present passthrough originals as completed transcoding. | C001,C002 | 04/06/48/49; F05 |
| C008 | High | Replace Workbench summary labels with interactive shared session panels, real docking/resize/focus and complete profile selection. Connect content, history, plugin/settings and conversation controllers; preserve state across Focus/Workbench switches. | C002,C007 | 05/42/45/49/51/53; F08 |
| C009 | High | **Implemented; see ticket 50 follow-up.** Make agent stale-overlap behavior and undo match decision 08: last-writer-wins per admitted batch, diagnostics for losing intent, independent undo for each batch. Add regressions replacing the contradictory conflict/grouped-agent tests; preserve human undo grouping and cloud conflicts. | C001 | 08/50 override; F07 |
| C010 | High | Replace AgentAdapter/McpSurface no-ops with the existing scoped AgentChannel/McpClient and a bounded host transport. Inject real validation feedback, host actor attribution and cancellation; connect provider/model/session selection and native progress/reference UI. | C008,C009 | 08/50–52; F06 |
| C011 | High | Define closed guest action/event/SDK dispatch for each admitted data/query/network/workflow capability. Wire handlers to existing host services and enforce per-principal/env/resource budgets, cancellation and redaction. Prove via a real WASM guest, not direct host-method calls alone. | C001,C004 | 04/15/16/19/20/26/27; F11 |
| C012 | High | Specify missing declarative extension content-type/validator/migration contracts and the capability catalog; implement admission, consent/revocation, actual sandbox hook preemption/memory enforcement, and native installation/settings/removal with tracked commands. | C008,C011 | 04/07/35/53; F12 |
| C013 | High | Complete native identity/dashboard/settings/help/about/conflict/recovery screens using their existing controllers. Add native input tests for catalog modes/lifecycle, preferences taking effect, explicit feedback consent, recovery and keyboard navigation. | C001,C008 | 54–58; F09 |
| C014 | High | Supply Studio Cloud identity/sync/asset-transfer deployment contract and test service, then replace OfflineSyncWorker in the product. Verify registration/email flow, offline outbox replay, disable/resume, pre-open conflicts and snapshot+journal recovery. | C001,C007,C013; service/configuration | 09/57/58; F13 |
| C015 | High | Implement and run the complete local Designer acceptance journey (ticket 69): fresh identity/project, real canvas edits, token/asset/profile, prototype, signed build, Runtime interaction and cold reopen; retain evidence and adversarial cases. | C001–C008,C013 | 04/10/12/39; F02/F03/F05/F08/F09 |
| C016 | High | Connect `studio dev` to direct Rust IR interpretation/hot swap without per-edit Wasm compilation, or explicitly amend spec 004 before accepting the alternative. Verify ten edits, invalid rollback, compatible state preservation and source identity. | C002,C004 | spec 004 FR-004/FR-011; F04 |
| C017 | Medium | Reconcile remaining spec 005–008 full-gate/real-package evidence and spec 009–014 acceptance status; update release docs and remove stale code-only/missing-libclang conclusions where current proof supersedes them. No unchecked live/native gate is closed by a model test. | C015,C016; approved test environment | F16 |
| C018 | High / external | Provision safe staging and OAuth registrations; run REST/SSE/WSS/provider/Stripe suites and physical three-station/peripheral journey. Record only safe codes/metadata; missing credentials or hardware remains a named blocker or explicit waiver. | C010–C012,C014,C015; operator credentials/devices | 19–21/34/36/59/64; F14 |
| C019 | High / release | Run full application builds and native acceptance on supported OS/display backends; produce signed release artifacts, installer/update/rollback rehearsal and compressed/installed/app-payload size evidence. Complete accessibility, dependency/provenance/license and release-owner sign-off. Keep deferred low-power certification explicit. | C003,C015,C017,C018; release checkpoint | 12, Constitution V, release checklist; F15 |
| C021 | High / verification | **Complete; remote run 41285190138 passed.** Restore Linux CI after the observed runner `No space left on device` failure. Apply consistent verification profile/incremental settings before Cargo, measure artifact/disk usage, keep lint/test/release stages serialized and budgeted, and obtain a fresh full successful run. Preserve failure evidence; do not delete unrelated developer caches or claim a failed runner proves test success. | CI runner/configuration | AGENTS policy; F17 |
| C020 | Planned feature | Specify and implement the trusted native sidecar bridge and Rust/Go/C Media Pipeline Lab from the saved proposal. Stabilize protocol/lifecycle/conformance before media, native video output and packaging; preserve the WASM authority boundary. | Explicit feature contracts; C011/C019 as integration dependencies | Native sidecar proposal; additional scope |

## Concrete first tests and code boundaries

- C001: `crates/studio-designer/tests/project_identity_isolation.rs` (new) and host persistence
  tests; relevant sources are `bootstrap.rs`, `project_dashboard.rs`, and
  `studio-host/src/designer_persistence.rs`. First prove the collision/foreign-open before fixing it.
- C002/C003: add native GPUI UI integration tests using the production view, not include_str
  assertions. Relevant files: `studio-designer/src/focus_view.rs`, Runtime foundation rendering,
  `studio-components/src/catalog.rs`, and catalog mapping tests. Read GPUI Kit guides before UI work.
- C004–C006: define the versioned contract, then add `design_build_launch` tests at the shared
  compiler/host boundary. Reuse `studio-script`, `studio-package`, and Runtime admission; never
  send signing secrets into a guest or write a second ad-hoc archive implementation.
- C007–C014: extend the existing Library/content/agent/MCP/registry/connection/resilience suites
  and add production-view tests for the call paths that are currently absent.
- C015: `crates/studio-designer/tests/authoring_journey.rs` (new) plus target-platform native
  runs from the acceptance runbook. Record independently which steps are deterministic versus native.

## Verification commands

During a slice, choose only affected targets, for example:

```sh
cargo test --locked -p studio-designer --test project_identity_isolation
cargo test --locked -p studio-host --test designer_persistence
cargo test --locked -p studio-design --test agent_channel
cargo test --locked -p studio-design --test mcp_client
cargo test --locked -p studio-components --test catalog_mapping
```

New target names above are planned tests, not commands that work before their implementation.
At integration checkpoints run formatting, workspace tests, and workspace Clippy sequentially;
run release builds only at an explicit release checkpoint. Follow the debug/incremental/cache
policy in `AGENTS.md`. Reuse existing evidence until code or unresolved findings justify a rerun.

## Completion rule

A task is complete only when its acceptance behavior works through the intended production path
and its evidence is recorded. Domain APIs, placeholders, fake provider fixtures, successful
compilation, and source-string checks must not be reported as an executed native user journey.
