# Feature Specification: Studio Toolchain and Development Workflow

**Feature Branch**: `003-studio-toolchain`

**Created**: 2026-09-03

**Status**: Draft

**Input**: User description: "`studio` (`crates/studio-cli`) owns build/dev, `studio-app` owns
runtime; move packaging to Rust; keep TS scripts as shims; define build phases/diagnostics/exit
codes; add debounced watcher that ignores `assembly/routes.generated.ts` (currently
self-triggers) and handles cancellation/one-build-at-a-time; tests for clean/failed/repeated
builds."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - One-Command Reproducible Builds (Priority: P1)

A plugin author runs the Studio toolchain's single build command on a plugin project and either
receives a runnable bundle or a structured failure that names the phase, a stable diagnostic code,
and a safe message. Rebuilding the same inputs produces the same bundle, and a failed build never
leaves output that could be mistaken for a fresh success.

**Why this priority**: Every other workflow (dev loop, packaging, quickstart, CI) depends on a
build command that is trustworthy, deterministic, and diagnosable.

**Independent Test**: Build the starter example from a clean state, verify the bundle is
produced; corrupt a source file and verify a structured failure with no partial bundle; rebuild
unchanged inputs and verify identical artifacts.

**Acceptance Scenarios**:

1. **Given** a valid plugin project, **When** the author runs the build command, **Then** a
   runnable bundle is produced without invoking any runtime host process.
2. **Given** a project with a compile or validation error, **When** the build runs, **Then** it
   exits with the documented failure code, reports the failing phase and stable diagnostic code,
   and produces no new bundle.
3. **Given** two builds with identical inputs, **When** both complete successfully, **Then** the
   produced bundles are byte-identical.
4. **Given** a build interrupted mid-run, **When** the author inspects the output location,
   **Then** no partial bundle is present and the previous good bundle (if any) is untouched.

---

### User Story 2 - Trustworthy Development Watch Loop (Priority: P1)

A plugin author runs the Studio dev command, edits sources in an editor, and the toolchain
rebuilds exactly when real sources change: generated-file churn never re-triggers a build, rapid
successive saves coalesce into one build, at most one build runs at a time, and cancelling the
dev command stops cleanly without corrupting state.

**Why this priority**: The watcher currently self-triggers on generated files and can overlap
builds; the dev loop is the author's primary feedback channel and must be dependable before
packaging work builds on it.

**Independent Test**: Run the watcher against a fixture project, emit generated-file churn,
rapid saves, and a cancellation; verify coalesced builds, no self-triggering, single-flight
execution, and clean shutdown.

**Acceptance Scenarios**:

1. **Given** a running dev session, **When** a generated route-binding file is rewritten with
   identical or generated content, **Then** no build is triggered.
2. **Given** five source saves within the debounce window, **When** the window closes, **Then**
   exactly one build runs for the final state.
3. **Given** a build already running, **When** a new source change arrives, **Then** the change
   is superseded or queued deterministically and builds never run concurrently.
4. **Given** a dev session, **When** the author cancels it, **Then** the in-flight build stops
   cleanly, no partial bundle remains, and a subsequent dev session works normally.

---

### User Story 3 - Toolchain-Owned Packaging (Priority: P2)

Packaging — bundle assembly, manifest generation, and the existing signing/trust admission — is
performed by the native toolchain itself. The existing TypeScript script entry points keep
working unchanged as thin shims that delegate to the toolchain, so author workflows and docs do
not break during the move.

**Why this priority**: Packaging correctness is already covered by existing trust admission;
moving ownership removes a script/toolchain split, but authors are not blocked without it.

**Independent Test**: Run each documented TS script entry and the equivalent native command on
the same inputs and verify identical bundles; run the packaging failure family and verify the
same structured diagnostics.

**Acceptance Scenarios**:

1. **Given** a valid project, **When** packaging runs through the native command, **Then** the
   produced bundle matches the bundle produced by the existing documented script path.
2. **Given** the documented script entries, **When** they are invoked, **Then** they delegate to
   the native toolchain and produce identical results and exit codes.
3. **Given** a packaging failure (missing manifest input, signature failure), **When** it occurs,
   **Then** the failure names the packaging phase with a stable diagnostic code and the same
   exit-code family as other build failures.

---

### User Story 4 - Automation-Friendly Diagnostics (Priority: P2)

CI pipelines and editor integrations can classify every build outcome using documented exit codes
alone, and can read machine-readable diagnostics that name the phase, stable code, and safe
message for each problem.

**Why this priority**: Deterministic automation unblocks CI gating and future editor tooling, but
authors can work manually without it.

**Independent Test**: Run the documented failure families and verify the exit code alone
classifies each outcome identically to the diagnostics, with zero ambiguity between families.

**Acceptance Scenarios**:

1. **Given** each documented failure family, **When** a build fails that way, **Then** the exit
   code uniquely identifies the family.
2. **Given** a failed build, **When** machine-readable diagnostics are requested, **Then** every
   diagnostic names its phase, stable code, and safe message.
3. **Given** a successful build, **When** it completes, **Then** the success exit code is
   distinguishable from every failure family.

### Edge Cases

- Editor atomic saves (write-rename churn) must not produce phantom builds or missed changes.
- A watched directory disappearing or becoming unreadable must surface a structured diagnostic
  instead of crashing the dev session.
- Deleted or unwritable output locations must fail in the packaging phase with a stable code and
  leave no partial artifacts.
- Builds run while a previous bundle exists must replace it atomically or fail without damaging
  the existing bundle.
- Concurrent invocations of the build command on the same project must be prevented or serialized
  with a clear diagnostic rather than corrupting output.
- The watcher must behave correctly when a save produces both a source change and a generated-file
  change: exactly one build for the source change, no build for the generated churn.
- Interrupted dev sessions (host sleep, process kill) must not leave lock state that blocks the
  next session.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The toolchain MUST provide one build command that produces a runnable bundle from a
  plugin project or fails without producing a new bundle.
- **FR-002**: Builds MUST execute as named phases (at minimum: validate, compile, package) and
  every diagnostic MUST identify its phase.
- **FR-003**: Every diagnostic MUST carry a stable code and safe message, and every outcome MUST
  map to a documented, unique exit code family.
- **FR-004**: Repeated builds from identical inputs MUST produce byte-identical bundles.
- **FR-005**: Failed or interrupted builds MUST NOT leave a partial bundle and MUST NOT damage an
  existing good bundle (atomic output).
- **FR-006**: The dev watcher MUST NOT trigger builds from generated-file churn, including the
  generated route-binding file that currently self-triggers.
- **FR-007**: The dev watcher MUST debounce rapid saves and run at most one build at a time; a
  change arriving during a build MUST be superseded or queued deterministically.
- **FR-008**: Cancelling a dev session MUST stop the in-flight build cleanly, leave no partial
  output or blocking lock state, and allow a subsequent session to run normally.
- **FR-009**: Packaging MUST be owned by the native toolchain, and each documented TypeScript
  script entry MUST continue to work as a shim that delegates to it with identical results and
  exit codes.
- **FR-010**: The dev command MUST present the plugin surface through the shared runtime host
  components rather than launching the production runtime binary.
- **FR-011**: The toolchain MUST provide machine-readable diagnostics (one structured record per
  problem) suitable for CI and editor consumption.
- **FR-012**: Clean, failed, and repeated build paths MUST be covered by automated tests, per the
  project's test-first constitution.

### Key Entities

- **BuildRequest**: A project plus requested profile; the input to one build.
- **BuildPhase**: A named stage (validate, compile, package) that owns its diagnostics.
- **BuildDiagnostic**: One safe record: phase, stable code, message, optional source location.
- **ExitCodeFamily**: The documented mapping from outcome classes to process exit codes.
- **WatchSession**: The dev loop state: watched sources, debounce policy, in-flight build, and
  supersede/queue decisions.
- **Bundle**: The runnable output artifact produced atomically by packaging.
- **Shim Contract**: The guarantee that documented script entries delegate to the toolchain with
  identical results.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A plugin author can build the starter example with one command from a clean state
  in under one minute on the development validation host.
- **SC-002**: Two builds from identical inputs produce byte-identical bundles in all tested
  cases.
- **SC-003**: A watch session exercised with 50 rapid successive saves (including generated-file
  churn) produces exactly the expected coalesced builds and zero self-triggered builds.
- **SC-004**: Every tested failure exits with a documented code that uniquely identifies its
  family, with zero partial bundles observed in any failure case.
- **SC-005**: All documented TypeScript script entries produce identical bundles and exit codes
  to their native equivalents on the same inputs.
- **SC-006**: Automated tests cover the clean, failed, and repeated build paths plus watcher
  debounce, single-flight, cancellation, and generated-file exclusion.

## Assumptions

- `studio-app` remains the runtime and host authority; the toolchain gains no runtime hosting
  duties beyond presenting dev surfaces through the shared host components.
- Signing and trust admission reuse the existing packaging trust model unchanged; no new trust
  semantics are introduced.
- Existing TypeScript scripts remain supported shims for the foreseeable future; deleting them is
  out of scope.
- The watcher observes a local filesystem; network filesystems and remote edit flows are out of
  scope.
- Performance targets apply to the development validation host, consistent with prior features.
