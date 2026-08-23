<!--
Sync Impact Report
- Version change: unratified template -> 1.0.0
- Modified principles:
  - PRINCIPLE_1 -> I. Host Authority and Zero Trust
  - PRINCIPLE_2 -> II. Test-First Evidence (NON-NEGOTIABLE)
  - PRINCIPLE_3 -> III. Versioned Contracts Before Implementations
  - PRINCIPLE_4 -> IV. Retained Native UI and Host-Scheduled Work
  - PRINCIPLE_5 -> V. Wayland-Only, Minimal Native Surface
- Added principles:
  - VI. Small, Traceable Vertical Slices
- Added sections:
  - Security and Resource Constraints
  - Development Workflow and Quality Gates
- Removed sections: none
- Dependent templates: no edits required; Spec Kit reads this constitution at runtime
- Follow-up TODOs: none
-->
# Studio Runtime Constitution

## Core Principles

### I. Host Authority and Zero Trust

The Rust host MUST retain exclusive authority over native rendering, navigation, secrets,
filesystem access, networking, hardware, persistence, and capability decisions. Guest plugins
MUST execute in Wasmtime without WASI and MUST communicate only through the documented,
versioned Studio ABI. Every plugin, bundle, message, pointer, length, identifier, property, and
action request MUST be treated as untrusted, including input from a validly signed publisher.

Secrets MUST remain in host-owned memory. Guest code may receive only opaque, scoped,
short-lived references that are bound to a plugin principal, instance, purpose, and session.
Host-owned confirmation surfaces MUST mediate security-sensitive actions.

Rationale: the sandbox boundary is a product guarantee, not an implementation detail.

### II. Test-First Evidence (NON-NEGOTIABLE)

Every behavioral change MUST follow Red-Green-Refactor:

1. Add or update an automated test that expresses the required behavior.
2. Run it and confirm it fails for the expected reason.
3. Implement the smallest correct change that makes it pass.
4. Refactor only while the focused and affected test suites remain green.
5. Run the feature's documented verification commands before marking its task complete.

Bug fixes MUST first reproduce the defect with a failing test. Security boundaries MUST include
negative and adversarial tests, not only success cases. Protocol changes MUST include
serialization, validation, and compatibility tests on both sides of the host-guest boundary.

Documentation-only and mechanically generated changes MAY omit a new failing test when they
cannot change runtime behavior, but their generation, formatting, or consistency checks MUST
still pass. A task is not complete because code compiles or appears correct; repeatable evidence
is required.

Rationale: executable proof prevents architectural intent from drifting during implementation.

### III. Versioned Contracts Before Implementations

Rust types in `studio-protocol` are the authoritative host-guest contract. JSON Schema,
AssemblyScript bindings, fixtures, and documentation MUST be generated from or checked against
that source. Public messages, UI nodes, properties, events, actions, bundle manifests, routes,
and error codes MUST use closed, versioned schemas with explicit limits.

An implementation MUST NOT accept undocumented imports, fields, node kinds, properties,
capabilities, or protocol proposals. Breaking contract changes require a protocol version change,
migration notes, compatibility fixtures, and an explicit rollout plan. Patch batches MUST be
fully validated before they mutate host state and MUST apply atomically.

Rationale: a narrow, deterministic contract is necessary for security, multiple SDKs, and a
future web host.

### IV. Retained Native UI and Host-Scheduled Work

Plugins MUST emit an initial declarative UI tree and targeted structural or property patches.
The host MUST own native component construction, layout, focus, accessibility, overlays,
navigation, animation clocks, and frame scheduling. Milestone-one plugins MUST NOT receive an
immediate-mode canvas, arbitrary native widget handles, shaders, or a per-frame callback.

Property updates MUST invalidate only the affected native state where practical. Structural
patches MAY be more expensive but MUST preserve node identity and interaction state whenever the
protocol operation permits it. An idle guest MUST not consume CPU to redraw unchanged content.

Rationale: retained host rendering provides native behavior and bounded work proportional to
state changes.

### V. Wayland-Only, Minimal Native Surface

The initial host MUST run natively on Wayland and MUST NOT compile, link, or silently fall back
to X11 or XWayland. CI MUST inspect both the Cargo feature graph and linked binary for X11/XCB
capabilities. Startup without a native Wayland endpoint MUST fail with a controlled diagnostic.

Dependencies and feature flags MUST be deny-by-default. A new native subsystem, Wasmtime import,
capability, network path, storage path, or hardware adapter requires a specification, threat-model
update, focused tests, and evidence that the existing narrow surface cannot satisfy the need.

Rationale: platform and dependency constraints are enforceable only when they are continuously
tested.

### VI. Small, Traceable Vertical Slices

Implementation MUST proceed through dependency-ordered tasks that deliver independently
verifiable behavior. Each task MUST identify its requirement or user story, expected files,
acceptance behavior, first failing test, and verification command. Tasks SHOULD touch no more
than five files unless the plan documents why an atomic cross-cutting change is safer.

The host contract, SDK behavior, native mapping, and example plugin MUST evolve together when a
feature crosses those layers. Discovered constraints, failed assumptions, security findings, and
scope changes MUST update the active specification before implementation continues.

Rationale: small slices make reviews, rollbacks, failures, and specification drift manageable.

## Security and Resource Constraints

- Signed bundles MUST pass size, path, schema, digest, signature, identity, capability, import,
  and export validation before compilation or instantiation.
- Developer mode MAY bypass signature trust only when explicitly enabled; it MUST retain all
  runtime, protocol, resource, and capability limits and MUST display persistent untrusted state.
- Wasmtime stores MUST enforce memory, table, fuel, epoch, message, tree, queue, and feature
  limits. Host-created limits MUST also cover module-defined resources.
- Guest memory access MUST use checked pointer arithmetic, bounded copies, UTF-8 validation where
  required, and no retained guest slices across calls.
- Raw secrets and active opaque handles MUST NOT enter logs, telemetry, crash reports, snapshots,
  receipts, or persistent storage.
- Money MUST use integer minor units and an explicit currency. Floating-point values MUST NOT
  represent transaction amounts.
- Capability operations MUST be declared by the signed manifest and authorized for the current
  principal. Unknown or undeclared capabilities MUST fail without prompting.
- Milestone-one payment and printer actions MUST remain deterministic simulators with no external
  network or real hardware access.
- Resource budgets and performance targets in the active specification are acceptance criteria,
  not optional optimization goals.

## Development Workflow and Quality Gates

Significant features and architectural changes MUST use the Spec Kit workflow:

1. `speckit-specify` defines observable requirements and independent acceptance scenarios.
2. `speckit-clarify` resolves material ambiguity before planning. It is mandatory for changes to
   trust boundaries, secrets, signing, capabilities, payments, hardware, or protocol compatibility.
3. `speckit-plan` defines architecture, data flow, contracts, failure modes, and verification.
4. `speckit-tasks` produces dependency-ordered, test-first vertical slices.
5. `speckit-analyze` checks artifact consistency before security-sensitive implementation.
6. Tasks are implemented individually under Principle II, not accepted as an unverified bulk pass.
7. `speckit-converge` compares delivered behavior with the specification and appends any remaining
   work before the feature is declared complete.

Every change MUST pass applicable formatting, linting, unit, contract, integration, security, and
build checks. Native releases additionally MUST pass Wayland-only feature and linkage checks.
Manual smoke tests supplement automation but MUST NOT replace an automatable assertion.

Reviews MUST verify constitutional compliance, specification traceability, failure handling,
resource limits, and the absence of secret exposure. Complexity, new dependencies, exceptions,
and deferred tests require written justification in the active plan or specification.

## Governance

This constitution supersedes conflicting development conventions, templates, plans, and local
preferences. Product requirements remain authoritative only when they do not weaken these
governance guarantees; conflicts MUST be resolved by amending the constitution or revising the
requirement before implementation.

Amendments require:

1. A written proposal describing the motivation and affected principles.
2. Impact analysis covering active specifications, code, tests, security assumptions, and
   migration needs.
3. Explicit user or maintainer approval.
4. A semantic version update and ISO amendment date.
5. Any required migration tasks before dependent feature work proceeds.

Versioning follows semantic versioning: MAJOR for incompatible removals or redefinitions of
governance guarantees, MINOR for new principles or materially expanded obligations, and PATCH
for non-semantic clarification. Each feature plan and review MUST include a constitution check.
Exceptions MUST be temporary, narrowly scoped, documented with an owner and expiry condition,
and MUST NOT weaken secret isolation or the host-guest trust boundary.

**Version**: 1.0.0 | **Ratified**: 2026-08-03 | **Last Amended**: 2026-08-03
