# Feature Specification: Studio Script and Embedded Development Host

**Feature Branch**: `004-studio-script`

**Created**: 2026-09-03

**Status**: Draft

**Input**: User description: "Rust + `rsvelte` pipeline: parse `.studio` sources into a
Svelte-compatible AST, validate and lower them into typed Studio IR, interpret that IR in the
Rust development runtime for identity-preserving HMR, and lower the same IR to AssemblyScript
for production ASC → Wasm builds. QuickJS is not part of this architecture."

## Clarifications

### Session 2026-09-03

- Q: When a dev-session edit changes a state slot's type, should the runtime convert the value
  or reset it to the new slot's initial value? → A: The language has one canonical numeric
  type, so numeric migrations do not exist; a slot whose type genuinely changes resets to its
  initial value; language-to-host numeric mapping (Integer vs Decimal) is deterministic and
  documented once.

## User Scenarios & Testing

### User Story 1 - Author a Studio Component Once, Run It Natively (Priority: P1)

A plugin author writes a `.studio` component using the Studio Script language (Svelte-style
markup whose tags name the closed Studio component catalog, plus a `<script>` block using
Studio-approved runes and typed state). The toolchain validates it against the portable
language subset and reports precise diagnostics for anything unsupported. A valid component
compiles to Studio IR and renders through the native host with no manual IR, AssemblyScript, or
host knowledge required.

**Why this priority**: Without a validated authoring path that renders natively, none of the
development runtime, HMR, or production lowering has a user.

**Independent Test**: Compile the reference `ProductCard` component (props with defaults,
state, derived values, conditionals, keyed iteration, interpolation, typed events) and verify
the rendered native surface and emitted protocol messages match recorded golden output.

**Acceptance Scenarios**:

1. **Given** a valid `.studio` component, **When** it is compiled, **Then** the produced Studio
   IR carries stable module/component/node identities, typed props and state slots, and no
   raw source text in executable nodes.
2. **Given** a component using a construct outside the portable subset (browser API, `eval`,
   dynamic import, `any`, untyped closure), **When** it is compiled, **Then** compilation fails
   with a diagnostic naming the construct, its stable code, and the exact source span, and no
   IR is produced.
3. **Given** a valid component, **When** it is interpreted by the development runtime, **Then**
   the retained native tree matches the declared template and emits exactly the declared typed
   events.

---

### User Story 2 - Identity-Preserving Hot Swap While Developing (Priority: P1)

During `studio dev`, an author edits and saves a `.studio` source file. The development runtime
parses, validates, and lowers the edited module, then atomically replaces the live component
implementation: retained native widgets keep their identity where the contract permits, state
slots with unchanged stable IDs and compatible types are preserved, new slots initialize, and
removed slots are disposed. An invalid edit keeps the last valid component running with a
structured diagnostic; nothing partially swaps.

**Why this priority**: Fast, safe feedback without rebuilding Wasm per keystroke is the core
value of the embedded development host.

**Independent Test**: Start a dev session on a stateful component, apply a sequence of edits
(valid tweak, invalid syntax, state-type change, component addition), and verify per-edit swap
outcomes: preserved state, retained node identity, rejection-and-continue behavior, and no
host crash.

**Acceptance Scenarios**:

1. **Given** a running dev session with interactive state, **When** the author saves a change
   to template markup only, **Then** the widget identities and state values are preserved and
   only the changed template surface updates.
2. **Given** a running dev session, **When** the author saves a syntactically or semantically
   invalid module, **Then** the swap is rejected, the last valid component keeps running, and
   the diagnostic names the problem with a stable code and span.
3. **Given** a running dev session, **When** the author changes a state slot to a genuinely
   different type (for example number to string, or a record shape change) or removes a
   component, **Then** the affected slots/nodes are disposed safely and re-initialized, numeric
   edits never count as type changes, compatible state is preserved, and the swap stays
   transactional (no partially applied replacement).
4. **Given** a module with imported dependencies, **When** it or a dependency is edited, **Then**
   dependents are reevaluated in dependency order within the same swap.

---

### User Story 3 - Production Wasm from the Same IR (Priority: P1)

A validated Studio module lowers to AssemblyScript source compatible with the existing Studio
SDK, and ASC compiles it to a Wasm module that the existing host admits and runs with the same
observable protocol behavior as the development runtime produced for the same inputs.

**Why this priority**: One language with two deterministic backends is the architecture's
contract; a Wasm gap would leave production builds broken.

**Independent Test**: Compile reference IR fixtures through both the Rust evaluator and the
AssemblyScript backend, run both artifacts against the protocol harness, and compare observable
messages for equivalence.

**Acceptance Scenarios**:

1. **Given** a valid component, **When** the production pipeline runs (`lower → ASC → Wasm`),
   **Then** the resulting Wasm is admitted by the existing host trust machinery and mounts the
   declared surface.
2. **Given** representative IR fixtures (literals, conditionals, keyed iteration, state
   transitions, typed events), **When** executed by the Rust evaluator and the compiled Wasm
   under identical event sequences, **Then** both emit identical observable Studio protocol
   messages in identical order.
3. **Given** an IR construct that cannot be lowered, **When** the lowering runs, **Then** it
   fails with a stable diagnostic rather than emitting non-compiling AssemblyScript.

---

### User Story 4 - Deterministic Build Lifecycle (Priority: P2)

The toolchain's start/resume/build/release lifecycle treats sources as authoritative and
generated artifacts (IR, AssemblyScript, Wasm) as fingerprinted caches: identical inputs and
toolchain versions produce byte-identical artifacts; stale caches rebuild automatically; edits
during dev mark the Wasm stale without forcing an ASC build.

**Why this priority**: Predictable caching makes CI and team workflows reliable, but authors can
work without it once US1–US3 exist.

**Independent Test**: Build a project twice and compare artifacts; touch a source and verify
cache invalidation only for the affected module chain; verify dev edits do not trigger ASC
rebuilds until an explicit build or session boundary.

**Acceptance Scenarios**:

1. **Given** identical inputs and pinned toolchain versions, **When** two builds run, **Then**
   the IR, AssemblyScript, and Wasm artifacts are byte-identical.
2. **Given** an edited source, **When** the next build runs, **Then** only the stale module and
   its dependents are recompiled.
3. **Given** a dev session with edits, **When** no explicit build is requested, **Then** no ASC
   compilation runs; the Wasm snapshot is refreshed once at the next session boundary or
   explicit build.

### Edge Cases

- A `.studio` file with no `<script>` block is a valid pure-template component; all template
  expressions must still satisfy the portable subset.
- Duplicate component names, duplicate node keys, and duplicate prop declarations within a
  module are compile-time diagnostics, not runtime errors.
- Recursive imports (module cycles) are detected and rejected with a diagnostic naming the
  cycle.
- A state slot initialized from another slot's value captures a snapshot; later mutations do not
  retroactively affect the initializer (documented derivation requires `$derived`).
- Keyed iteration over an unkeyed or non-array collection fails validation with guidance toward
  keyed arrays.
- Swapping a module whose dependency also changed in the same tick applies both atomically in
  dependency order or not at all.
- Cancelled dev sessions and killed processes leave no IR cache state that blocks the next
  session.
- Deeply nested templates and very long interpolations are bounded by the existing protocol
  limits with diagnostic overflow messages rather than stack exhaustion.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The compiler MUST parse `.studio` sources through the pinned `rsvelte` adapter,
  hidden behind the Studio compiler boundary so no `rsvelte` types escape `studio-script`.
- **FR-002**: The compiler MUST define and version a typed Studio IR (modules, components,
  props, state/derived slots, handlers, templates, expressions) whose stable IDs derive from
  module identity and structural/source identity.
- **FR-003**: The compiler MUST validate the portable language subset semantically (typed or
  inferrable bindings, serializable values, approved runes and SDK calls, closed catalog
  components) and MUST reject unsupported constructs with stable-coded spanned diagnostics at
  compile time.
- **FR-004**: The development runtime MUST interpret Studio IR directly in Rust (expressions,
  handlers, template projection onto the retained component registry) without JavaScript
  evaluation or per-edit Wasm rebuilds.
- **FR-005**: The development runtime MUST perform transactional, identity-preserving module
  replacement: parse/validate/lower before touching live state, reject invalid IR, preserve
  compatible state slots by stable ID and type, dispose removed slots, and roll back on host
  validation failure.
- **FR-006**: The production backend MUST lower validated Studio IR to AssemblyScript source
  that compiles with the pinned ASC against the existing Studio SDK, or fail with a stable
  diagnostic.
- **FR-007**: The Rust evaluator and the AssemblyScript/Wasm backend MUST produce equivalent
  observable protocol behavior for shared IR fixtures, proven by automated differential tests.
- **FR-008**: The module graph MUST resolve static imports across `.studio` modules, detect
  cycles, order dependent reevaluation, and feed deterministic build inputs.
- **FR-009**: Build artifacts MUST be fingerprinted over source contents, dependency graph,
  language/IR version, SDK/binding versions, `rsvelte` revision, ASC version/options, and
  target ABI; identical fingerprints MUST yield byte-identical artifacts.
- **FR-010**: The dev lifecycle MUST NOT require network access or external agent services for
  parse, validation, hot swap, or ASC compilation after dependencies are installed.
- **FR-011**: `studio dev` MUST reuse the shared host/runtime components rather than duplicating
  the production runtime binary (launch-seam composition per feature 003's research R5).
- **FR-012**: Every behavioral change MUST be covered by red-green tests per the constitution,
  including malformed input, determinism, invalidation, and differential fixtures.

### Key Entities

- **StudioModule**: One compiled `.studio` file: identity, imports/exports, component
  definitions, dependency list, source fingerprint.
- **ComponentDefinition**: Name, typed props, state/derived slots, event handlers, template
  nodes, span, stable component ID.
- **TemplateNode**: Closed tree of component/interpolation/conditional/keyed-iteration nodes
  with stable node IDs and spans.
- **Expression**: Closed, typed expression tree over literals, prop/state/derived/local reads,
  operators, records/arrays, and approved function calls; raw source text is never executable.
- **StateSlot / DerivedSlot**: Named, typed reactive values with stable IDs; derived slots
  recompute from dependencies; state slots persist across compatible swaps.
- **StudioType**: Closed type system governing validation, swap compatibility, and lowering.
  Numbers form one canonical type (`number`, JavaScript-like): an author editing `$state(1)` to
  `$state(1.5)` changes a value, never a type. When a number crosses into the host contract, the
  validator maps integral values within i64 range to Integer and all other numbers to Decimal,
  deterministically; exact-decimal money rendering goes through explicit formatting helpers,
  never float arithmetic.
- **ModuleGraph**: Directed import graph with cycle detection and dependency-ordered
  invalidation.
- **ArtifactFingerprint**: Versioned cache key binding artifacts to all toolchain inputs.
- **SwapTransaction**: Atomic old→new module replacement with state-migration outcome.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The reference component set (props/defaults, state, derived, conditionals, keyed
  iteration, interpolation, typed events) compiles to IR with zero diagnostics and renders
  natively with golden-matching protocol output.
- **SC-002**: Every rejected construct family has a stable diagnostic code and an exact source
  span, with zero IR emitted for the invalid module.
- **SC-003**: A 10-edit dev session (valid tweaks, one invalid edit, one incompatible state
  change) completes with preserved state where compatible, retained node identity, and no
  partial swaps; each swap completes within the existing interaction budget on the development
  validation host.
- **SC-004**: The differential fixture suite shows byte-identical observable protocol message
  sequences between the Rust evaluator and the ASC/Wasm artifact for every shared fixture.
- **SC-005**: Two identical builds produce byte-identical IR, AssemblyScript, and Wasm
  artifacts; an edit recompiles only the affected module chain.
- **SC-006**: A dev session runs fully offline (parse, swap, ASC) after dependencies are
  installed.

## Assumptions

- The pinned `rsvelte` revision (Rust workspace, github.com/baseballyama/rsvelte) is vendored or
  git-pinned; its `Engine` facade (prepare → facts → RuntimeArtifact, schema-versioned
  fingerprints) is the parsing/analysis frontend, hidden behind `studio-script`.
- The existing AssemblyScript SDK remains the Wasm-side runtime contract; the lowerer maps IR
  onto its APIs and does not redefine the protocol.
- The closed component catalog and SDK type surface from features 001–003 are stable inputs.
- Virtual modules (`@studio/generated/routes`, `@studio/generated/assets`,
  `@studio/dev-runtime`, `@studio/bootstrap`) are host-provided; normal components are ordinary
  modules. The bootstrap may be materialized only for ASC builds.
- Editor tooling, language server, formatting, and linting integrations are feature 008 scope
  and excluded here.
- The web/browser target and its host adapter are out of scope.
- Performance targets apply to the development validation host; the dev swap budget reuses the
  existing interaction budget.
