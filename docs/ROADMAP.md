# Studio implementation roadmap

Updated 2026-11-01 from source baseline `e21ed734`. Current evidence and remaining work are in
[the completion audit](development/PROJECT_COMPLETION_AUDIT.md),
[the ordered backlog](../.scratch/studio-designer/completion-tasks.md), and
[the local acceptance journey](development/DESIGNER_ACCEPTANCE_JOURNEY.md).

## Current milestone

Deliver one native, offline-capable Designer journey from a new authenticated project through
real native editing/preview to a deterministic signed application admitted by Studio Runtime.
That journey is **not complete**: the Focus canvas still renders node labels, and no native
Design build/sign/launch bridge is connected. Project ownership/session isolation is now fixed
with production-bootstrap and cold-restart regressions. Agent/MCP batch semantics now match
decision 08; native transport wiring remains open.

The original Wayland-only milestone is historical. Constitution 2.0.0 and
[ADR-0004](architecture/ADR-0004-native-platforms.md) permit upstream native backends, including
Linux Wayland/X11. The project uses published GPUI Kit 0.8.1 and the matching GPUI 0.3.10 family.
Windows/macOS reload socket tests and CLI checks have passed in CI; these do not certify full
native application builds, UI behavior, installers, or release support on those platforms.

## Runtime and toolchain feature status

| Feature | Present implementation / evidence | Remaining acceptance |
| --- | --- | --- |
| 001 Secure Plugin Runtime | Sandbox, closed ABI/protocol, retained registry, signatures/trust, simulators and deterministic tests. | Current platform, security/accessibility and release certification; broader capability routing assessed separately. |
| 002 Native Component Platform | Catalog, schemas/SDK, mappings and broad Runtime renderer implementation. | TimePicker deliberately fails catalog certification; full native inspector/input/preview parity remains. |
| 003 Toolchain | Phased compile/package pipeline, deterministic archives, watcher, structured diagnostics and tests. | Shipped watcher currently launches a Runtime child; do not describe it as an embedded IR host. |
| 004 Studio Script | Rust rsvelte frontend, typed IR, evaluator/hot-swap, ASC backend, module graph and differential tests. | FR-004/FR-011 integration gap: `studio dev` still builds/reloads Wasm rather than using direct IR interpretation. |
| 005 Reload | Runtime/CLI local channel, swap planner, request/revision handling and Unix/Windows adapter tests. | One unchecked full-gate task; native same-window behavior and complete acceptance evidence remain. |
| 006 Routing | Route graph/resolver/generation and parity tests. | One unchecked full-gate task. |
| 007 Asset imports/icons | Typed collector, staged assets, overrides/budgets, deterministic CLI pipeline. | Real-package icon test is ignored by default; US3/full-gate reconciliation remains open. Documentation status reconciled in this audit. |
| 008 Developer experience | Rust LSP, formatting/lint/projection/intelligence suites and thin VS Code client. | Full gate and editor smoke acceptance remain open; historical validation is not current native UX certification. |
| 009 Interactions | Counter evaluator/Wasm interaction differential test exists. | Acceptance reconciliation pending; Draft does not mean no implementation exists. |
| 010 Shape validation | Shared protocol cardinality/property rules and `STUDIO015` CLI diagnostics tests. | Acceptance reconciliation pending. |
| 011 Scaffolding | Template gallery and `studio new` tests. | Full acceptance reconciliation pending. |
| 012 Template publishing | Deterministic `studio build --as-template` tests and separated template bundles. | Full acceptance reconciliation pending. |
| 013 IDE templates | Thin VS Code gallery/command implementation and tests. | Actual editor smoke verification and acceptance reconciliation pending. |
| 014 Structural patches | Array mutations, keyed rows and structural evaluator/backend code paths exist. | Full list differential/native acceptance reconciliation pending; the original manual cart scenario is not certified here. |

Specs live in `specs/001-secure-plugin-runtime/` through `specs/014-structural-patches/`.
Task checkboxes remain acceptance-specific: this audit closes two documentation tasks in 007/008,
while five existing verification tasks across 005–008 remain unchecked. Existing checked tasks
in 004 do not override its source-confirmed shipped-path integration gap.

## Designer delivery order

1. **C001 / ticket 66 — implemented:** authenticated project ownership, restart-safe IDs and session disposal; 51 Designer and three host persistence tests pass.
2. **C002–C003 / 67:** shared native Runtime preview, real input/layout/property behavior and TimePicker certification.
3. **C004–C006 / 68:** immutable Design/Library build contract specified; production backend, signed artifact and native Build/Run remain open.
4. **C007–C013:** native Library/content/plugin/Workbench/support surfaces; agent/MCP transport/checker integration; guest capability dispatch. Agent conflict/undo semantics are implemented at the domain boundary.
5. **C014:** provision and integrate cloud identity/sync and recovery, preserving local/offline authority.
6. **C015 / 69:** run the complete local acceptance journey through production seams with recorded native evidence.
7. **C016–C019:** repair the IR development path, reconcile verification, run live services/hardware and close release gates.

Existing responsive/token/layout/prototype/session models and host services should be reused.
A tested adapter or controller must not be presented as a completed native screen. The domain
MCP client exists, but the shipped MCP surface is a no-op; the source audit names the exact gaps.

## Current CI blocker

The native Windows/macOS reload and CLI jobs passed for `e21ed734`. Linux validation passed
formatting/Clippy, then the runner exhausted disk during Rust tests. Full remote verification is
not green; C021 fixes the CI profile/artifact/disk budget and requires a fresh successful run.
The local 1,290-test checkpoint remains separate evidence.

## External release gates

[The release checklist](RELEASE_CHECKLIST.md) remains open for native keyboard/accessibility and
scaling, live OAuth/REST/SSE/WSS/Stripe evidence, the physical three-station/peripheral journey,
current dependency/provenance/license review, target-platform artifacts/signing/update rehearsal,
size measurements and release-owner approval. Formal low-power N100 certification is explicitly
deferred; it must not be reported as completed. The flagship fixture report stays not release-ready.

## Planned additional capabilities

The [native sidecar/media proposal](architecture/NATIVE_SIDECAR_BRIDGE_PROPOSAL.md) is tracked by
C020. It still needs contracts, host implementation, Rust/Go/C conformance examples and the media
backend/rendering/packaging work. Web/mobile Designer hosts are outside the currently accepted
Designer v1 scope; new Runtime platform delivery needs its own explicit requirements/evidence.

## Execution discipline

Use the existing Designer issue tracker for this project-wide handoff. For a feature managed by
Spec Kit, follow its spec/plan/tasks workflow. Start behavior changes with a failing regression,
verify focused targets during a slice, and use serialized formatting → workspace tests → Clippy
at integration checkpoints. Use the parent shared Cargo target/configuration policy in `AGENTS.md`.
Release builds run only at explicit release checkpoints. Do not close external/native gates by
substituting deterministic fixtures or source-string assertions.
