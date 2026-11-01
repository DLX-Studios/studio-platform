# Designer acceptance journey — local authoring to Runtime

Audit baseline: `e21ed734`, 2026-11-01. Result: **blocked by implementation**.
This runbook defines one complete acceptance scenario. Source paths and existing test evidence
were inspected; no complete graphical journey or newly built signed Design artifact was run.

## Scenario and fixtures

Use a fresh temporary Studio data directory and two Local Identities, Alice and Bob. Alice
creates a small two-screen counter application: Text, Button, TextInput, an admitted local PNG,
a shared spacing/color token and phone/desktop variants. A button increments state and a second
interaction navigates to Details. The app needs no external provider account or network service.
Use an isolated host test publisher/key and trust store for deterministic signing tests; never
store plaintext signing/OS credential material in project files, logs, or a guest.

Cloud sync, payment, physical POS stations and native extensions have their own acceptance gates;
they are not prerequisites for proving this local authoring path.

## Journey ledger

| Step | Action and required observation | Current evidence | Audit outcome |
| --- | --- | --- | --- |
| J01 | Fresh launch → welcome → create Alice → authenticated dashboard; restart honors remembered state and wrong credentials are denied. | Native bootstrap/identity services and `studio-designer/tests/native_bootstrap.rs`; some tests inspect source strings. | Implemented layers; native keyboard/OS-credential run still required. |
| J02 | Create Counter as Alice; close/reopen; create another project after process restart. Bob cannot read/open/edit either project, even with an injected Alice project ID. Sign-out disposes the old editor. | Production bootstrap tests cover authenticated ownership, random IDs, foreign-ID denial, expired adapters, retained-session teardown and child-process cold reopen. | Host/lifecycle checks pass (ticket 66); physical GUI sign-out/reopen still belongs to J12. |
| J03 | Open the starter, see its authored text rendered by the actual Runtime control; select via canvas/hierarchy without changing the content's semantics. | Projection contains the text, but `FocusView::render_node` shows kind/ID labels in nested boxes. | **Blocked: F02 / 67.** Projected props are not the currently rendered preview. |
| J04 | Insert/configure controls and two screens; edit a value, undo, redo, restart and observe durable state and stable node IDs. | Session, manipulation, Focus and script-editor suites cover model/state operations. | Partial: production view interaction/visual geometry evidence required. |
| J05 | Import PNG with provenance; bind it to Image; duplicate import keeps one identity; referenced deletion lists usages and requires resolution. | Host Library admission/usage tests exist; native editor opens without a Library snapshot and has no connected import/bind view. | **Blocked: F05 / C007.** |
| J06 | Apply shared token, override locally, clear override; edit phone/desktop variants and compare actual layouts. | Token/responsive domain tests and Focus actions exist; current canvas does not render Runtime layout/styles. | Partial models; **blocked on J03** for visual parity and complete profile UI. |
| J07 | Enter prototype, press Counter, edit input and navigate to Details; source history remains unchanged by prototype execution. | Prototype state/trace and navigation tests exist; native canvas lacks the corresponding controls. | Partial model; **blocked on J03** for true input/preview behavior. |
| J08 | Switch Focus ↔ Workbench; Library, inspector, history, diagnostics and interaction panels remain usable through pointer and keyboard with saved geometry. | Workspace state is persisted; native Workbench currently renders summary labels. | **Blocked: F08 / C008.** |
| J09 | Check and Build the exact immutable Design + Library revision; diagnostics link back to nodes; repeat produces identical signed artifact bytes with an exact asset manifest. Failed build preserves source and last-good output. | CLI/compiler/package tests exist independently; no audited Designer build/sign command connects this source revision to them. | **Blocked: F03 / 68.** |
| J10 | Run that artifact through production signature/admission, see the same controls and responsive behavior, press Counter and navigate without reconstructing the app by hand. Tampered assets/signature and undeclared actions are denied. | Runtime launch/admission tests use constructed packages; no same-Design artifact from J09 exists in the native path. | **Blocked on J09**; simulation/fixture packages do not complete this step. |
| J11 | Close Runtime/Designer and cold reopen Alice's saved project; build/run again. Bob's source and state remain independent. | Store/session recovery tests exist; connected journey not established. | **Blocked on J02/J09/J10**; needs a composed persistence regression. |
| J12 | Repeat with keyboard only, scaling and reduced motion on supported native platforms/display backends; record actual controls, focus order, accessibility and rendering. | Existing unit/model assertions and separate reload socket/CLI platform CI. | **Not executed**; native application/assistive-technology gate remains. |

J02 now has executable host/lifecycle evidence. J03 and J09 still prevent this scenario from
being presented as a completed native authoring-to-runtime journey.

## Automation contract

Ticket 69 adds a composed `authoring_journey` integration target using the real identity,
persistence, command, projection, build, signature and Runtime admission seams. It must not use
hardcoded successful receipt strings or substitute a separately handwritten package for the
Design authored in this run. Native GPUI UI integration tests use the production view and input
handlers, with actual changed state/rendered values as assertions. Keep the trusted test publisher
and fixture files isolated and clean up on success/failure.

Record each step's execution layer independently:

- deterministic model/protocol/host integration;
- native GPUI input/render behavior;
- OS credential/display/device/service acceptance where applicable.

A green domain test does not mark an unexecuted native step as passed. Avoid adding a permanently
ignored or unconditional-success “end-to-end” test while the build or renderer bridge is missing.

## Evidence to retain per actual run

Record commit, OS/architecture/display backend, compiler/build profile, runtime/Kit versions,
fixture hashes, source revision and Library snapshot identity, artifact digest, signature key ID
(not key material), command results, step outcomes, tester/date and safe diagnostics. Keep
screenshots/recordings for native visual steps and links to the test/CI results. Any waiver must
name its owner, scope and acceptance consequence.

Manual launch, once the missing paths are implemented, starts with:

```sh
cargo run --locked -p studio-designer
```

That opens a real application; do not run it as an unattended substitute for the step ledger.
Release builds and platform packaging are explicit release-checkpoint work.
