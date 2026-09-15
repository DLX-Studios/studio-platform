# Feature Specification: Runtime Reload and Preview Protocol

**Feature Branch**: `005-reload-protocol`

**Created**: 2026-09-03

**Status**: Draft

**Input**: User description: "Define CLI-to-runtime reload protocol, atomic validation/instantiation, window-preserving swap, safe disposal, preserved/reset state, failed-reload recovery, and e2e edit→rebuild→reload tests. Replace the `refresh studio-app window` placeholder. Add a `restart-session` command for the future editor extension."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Edit, Rebuild, Reload Without Restarting (Priority: P1)

A plugin author runs `studio dev`, edits a source file, and watches the running application
surface update in the same window once the rebuild completes: the CLI notifies the runtime over
the reload channel, the runtime validates and instantiates the new bundle before touching the
live surface, and the swap preserves the window. An invalid bundle is rejected with a
structured diagnostic while the old surface keeps running.

**Why this priority**: This is the dev loop's missing half; without it every edit costs a
manual restart.

**Independent Test**: Drive a scripted session against a stub runtime over a real loopback
channel: valid reload swaps, invalid reload rejects with the old surface intact, and every
exchange matches the protocol contract.

**Acceptance Scenarios**:

1. **Given** a running dev session, **When** a rebuild produces a valid bundle, **Then** the
   runtime swaps to it in the same window and acknowledges with the new revision.
2. **Given** a running dev session, **When** a rebuild produces an invalid bundle, **Then** the
   runtime rejects it with phase, code, and message, and the old surface keeps running
   untouched.
3. **Given** a reload request, **When** the bundle fails to instantiate, **Then** no partial
   surface appears and the failure names the stage.

---

### User Story 2 - Failed Reloads Never Kill the Window (Priority: P1)

A new bundle that crashes on initialization or fails validation mid-swap never leaves a dead
or half-rendered window: the runtime falls back to the last-good surface (or a safe empty
state when none exists) and reports exactly what failed and where the session stands.

**Why this priority**: A dev tool that can brick its own window on every other edit destroys
trust in the loop.

**Independent Test**: Script init crashes, validation failures, and mid-swap errors against the
swap state machine; verify fallback surface, revision accounting, and diagnostics.

**Acceptance Scenarios**:

1. **Given** a live surface, **When** the replacement crashes during initialization, **Then**
   the live surface remains and the diagnostic identifies the crash stage.
2. **Given** a swap that fails validation after the old surface was marked for disposal,
   **When** the failure occurs, **Then** disposal is cancelled and the old surface resumes.
3. **Given** no valid bundle has ever loaded, **When** the first bundle fails, **Then** a safe
   empty state appears with the failure diagnostic (never a dead window).

---

### User Story 3 - Session Control Commands (Priority: P2)

The author (and later the editor extension) can restart a dev session cleanly and cancel a
watch loop without residue: `restart-session` disposes the current instance and
re-instantiates from the latest valid bundle; cancellation stops the loop, kills the runtime
child, removes the channel socket, and leaves no lock or partial state.

**Why this priority**: Session control is required by the 008 editor extension contract and by
authors recovering from wedged states, but the loop works without it.

**Independent Test**: Restart against a stub runtime and verify disposal-then-instantiate
ordering with revision continuity; cancel a session and verify socket removal and clean exit
code.

**Acceptance Scenarios**:

1. **Given** a running session, **When** `restart-session` runs, **Then** the old instance is
   disposed before the new one instantiates, and the revision advances exactly once.
2. **Given** a dev session, **When** it is cancelled, **Then** the runtime child stops, the
   channel socket is removed, no partial bundle exists, and the process exits 130.

---

### User Story 4 - Defined Preserved/Reset State (Priority: P2)

Reloads follow a documented state policy: the navigation route survives a reload when the new
bundle still declares it (otherwise the default route loads), while plugin instance state
always starts fresh from the new instantiation. Authors can predict exactly what a reload
keeps.

**Why this priority**: Predictability is the difference between a tool and a slot machine, but
authors can work with a documented policy whenever it lands.

**Independent Test**: Reload across route sets (preserved route, removed route) and verify the
resulting route plus fresh instance state in each case.

**Acceptance Scenarios**:

1. **Given** a session on route `/pos`, **When** a reload lands whose bundle still declares
   `/pos`, **Then** the surface restores `/pos` with a fresh plugin instance.
2. **Given** a session on route `/pos`, **When** a reload lands whose bundle no longer
   declares `/pos`, **Then** the surface loads the default route with a fresh instance.

### Edge Cases

- A reload request arriving while a previous reload is still swapping is queued
  deterministically (one trailing reload with the latest bundle), never run concurrently.
- A bundle that validates but references routes the old session cannot resolve loads with the
  default route and a warning diagnostic, not a failure.
- The runtime child dying unexpectedly ends the session with a structured watch-family
  diagnostic (existing 003 behavior), not a hang.
- A stale socket file from a killed session is detected (dead peer) and replaced, never
  blocking a new session.
- Reloads of unsigned development bundles keep development trust semantics; production
  admission rules never apply on the dev channel.
- Cancellation during an in-flight swap disposes the pending replacement first, then stops.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The toolchain MUST expose a CLI-to-runtime reload channel carrying versioned
  reload requests and outcomes over a local transport.
- **FR-002**: The runtime MUST validate (manifest, signature, trust, assets) and instantiate a
  replacement bundle before touching the live surface (two-phase: prepare, then commit).
- **FR-003**: The swap MUST preserve the OS window and Wayland surface; only plugin content
  changes.
- **FR-004**: The old instance MUST be disposed safely (fuel/epoch revoked, sessions closed)
  only after the replacement commits; disposal MUST be cancellable until commit.
- **FR-005**: Failed validation, instantiation, or commit MUST leave the last-good surface
  running (or a safe empty state when none exists) with a structured diagnostic.
- **FR-006**: Reloads MUST be single-flight with deterministic supersede: one trailing reload
  with the latest bundle.
- **FR-007**: Navigation route MUST survive reloads when still declared, else the default
  route loads; plugin instance state MUST always start fresh.
- **FR-008**: A `restart-session` command MUST dispose and re-instantiate from the latest
  valid bundle with revision continuity.
- **FR-009**: Cancellation MUST stop the loop and runtime child, remove the channel socket,
  leave no partial output or lock residue, and exit 130.
- **FR-010**: The dev-mode trust semantics MUST stay development-scoped; the reload channel
  MUST NOT admit production bundles or alter production admission.
- **FR-011**: Edit→rebuild→reload cycles MUST be covered by automated end-to-end tests.
- **FR-012**: Every behavioral change MUST be covered by red-green tests per the constitution.

### Key Entities

- **ReloadRequest**: Versioned request: bundle path, request id, base revision.
- **ReloadOutcome**: Accepted (new revision) or rejected (stage, code, message); restart and
  cancel variants.
- **SwapTransaction**: Prepare (validate + instantiate) → commit (swap + dispose old) or
  rollback (keep live, report).
- **SessionRevision**: Monotonic revision counter owned by the runtime; advances exactly once
  per accepted reload or restart.
- **ChannelEndpoint**: Local socket path with stale-socket detection and cleanup.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A scripted session of 10 reloads (7 valid, 2 invalid bundles, 1 crashing bundle)
  ends on the last valid surface with all 10 outcomes acknowledged and zero dead windows.
- **SC-002**: Every reload outcome carries stage, code, and message; invalid bundles never
  disturb the live surface in any tested case.
- **SC-003**: A reload storm (5 requests during one swap) results in exactly one trailing
  reload with the latest bundle.
- **SC-004**: Route preservation and reset behave per the policy in all tested route sets.
- **SC-005**: Cancellation always exits 130 with the socket removed and no partial output.
- **SC-006**: Automated e2e covers edit→rebuild→reload over a real channel with a stub
  runtime host.

## Assumptions

- `studio-app` remains the runtime authority; the protocol is local IPC, not guest ABI, and
  introduces no network, storage, or capability paths.
- The existing `studio-package` verification (manifest, signature, trust, assets) is reused
  for the prepare phase; no new trust semantics.
- Display-level reload in a real compositor stays covered by the manual/headless gate pattern;
  automated e2e runs against a stub runtime host over loopback.
- `.studio` IR hot swap (feature 004) remains the fast path; the reload protocol is the full-
  rebuild slow path and both share the preserve/init/dispose vocabulary.
