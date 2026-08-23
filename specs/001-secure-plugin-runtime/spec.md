# Feature Specification: Secure Native Plugin Runtime

**Feature Branch**: None (Git extension not installed)

**Created**: 2026-08-03

**Status**: Draft

**Input**: Create a Wayland-only native platform that loads isolated business plugins, renders
declarative interfaces, applies fine-grained reactive updates, protects secrets with opaque
handles, and demonstrates the complete model through a simulated point-of-sale checkout.

## Clarifications

### Session 2026-08-03

- Q: After a plugin is terminated because of a trap or resource-limit violation, how should the
  operator recover? → A: Show a host-owned error screen with a manual restart action that creates
  a fresh plugin instance without restoring plugin state.
- Q: If the Wayland compositor disconnects while Studio is running, what should Studio do? → A:
  Cancel pending actions, revoke secrets, terminate plugins, and exit cleanly without restoring
  the session automatically.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Launch a Trusted Business Plugin (Priority: P1)

A shop operator launches Studio with an administrator-provisioned production bundle path. Studio
verifies the bundle before it runs and presents its initial interface as native controls. The
operator can use keyboard and pointer input with touch-sized tap and drag controls without seeing
browser chrome or a web page. Direct multi-touch gestures are outside milestone one.

**Why this priority**: Every later workflow depends on safely loading a plugin and presenting a
usable native interface. This is the smallest demonstrable product slice.

**Independent Test**: Provision one valid example bundle at an explicit absolute local path,
launch Studio with that production bundle in a native Wayland session, and confirm that the
catalog screen accepts keyboard, pointer-click, scroll, and slider-drag input. Repeat with an
altered or undeclared bundle and confirm it is rejected without affecting the host.

**Acceptance Scenarios**:

1. **Given** a trusted bundle with a valid declaration, **When** an operator opens it, **Then**
   Studio verifies it before execution and displays its initial native screen.
2. **Given** a bundle whose declared or executable content was changed after signing, **When** an
   operator opens it, **Then** Studio refuses to run it and displays a host-owned explanation.
3. **Given** an explicitly selected unsigned development bundle, **When** Studio is started in
   developer mode, **Then** it runs with a persistent untrusted indicator and retains all
   isolation and resource controls.
4. **Given** Studio is not running in a native Wayland session, **When** it starts, **Then** it
   exits with a clear unsupported-platform message and does not fall back to X11 or XWayland.
5. **Given** the active Wayland compositor disconnects, **When** Studio loses the display
   connection, **Then** it cancels pending actions, revokes secrets, terminates plugin instances,
   and exits cleanly without automatically restoring the session.

---

### User Story 2 - Operate a Responsive Catalog and Cart (Priority: P2)

A checkout operator browses products, searches or filters the catalog, adds items to a cart,
changes quantities, and sees totals update immediately. Existing focus, scroll position, and
unrelated controls remain stable as individual values change.

**Why this priority**: It proves that third-party business logic can drive a practical native
workflow without rebuilding or losing the state of the surrounding interface.

**Independent Test**: Use a fixed product catalog to add, update, and remove cart lines while
recording visible results and retained interaction state. Verify totals against independently
calculated integer currency values.

**Acceptance Scenarios**:

1. **Given** a displayed catalog, **When** the operator adds a product, **Then** the cart count,
   line item, subtotal, tax, and total reflect the selection once.
2. **Given** a cart containing several products, **When** one quantity changes, **Then** only
   values dependent on that quantity visibly change and unrelated focus and scroll state remain.
3. **Given** a discount control, **When** its value changes repeatedly during one interaction,
   **Then** Studio presents the final correct price without stale intermediate state.
4. **Given** currency calculations involving fractional major units, **When** totals are derived,
   **Then** the displayed values match exact minor-unit arithmetic with no floating-point drift.

---

### User Story 3 - Complete a Protected Simulated Payment (Priority: P3)

An operator reviews the cart, enters a PIN into a trusted Studio-controlled field, confirms the
merchant and exact amount, and runs a simulated payment. The plugin can determine whether
authorization information is ready and can request the payment, but it can never read the PIN.

**Why this priority**: Checkout demonstrates Studio's central security promise: useful plugin
logic can coordinate a sensitive action without receiving the underlying credential.

**Independent Test**: Complete checkout using each documented simulator result and inspect all
plugin-visible messages, diagnostics, snapshots, and persisted artifacts to confirm that no raw
secret is present.

**Acceptance Scenarios**:

1. **Given** a ready cart and no captured PIN, **When** payment is requested, **Then** Studio
   refuses the request and leaves the checkout recoverable.
2. **Given** a PIN entered into the trusted field, **When** Studio informs the plugin that input is
   ready, **Then** the plugin receives only an opaque reference and descriptive non-secret state.
3. **Given** a valid reference and checkout session, **When** the operator confirms the displayed
   merchant, amount, currency, and simulator status, **Then** the simulator returns its
   deterministic result for that request.
4. **Given** an expired, reused, foreign-instance, wrong-session, or wrong-purpose reference,
   **When** a payment is requested, **Then** Studio rejects it without revealing whether any raw
   secret ever existed.
5. **Given** a repeated payment request with the same idempotency key, **When** it is processed,
   **Then** the original result is returned and no second simulated charge is created.

---

### User Story 4 - Navigate, Recover, and Produce a Receipt (Priority: P4)

An operator moves through catalog, cart, checkout, payment, and receipt screens using predictable
back and forward behavior. Payment failures offer a safe retry or return path. An approved
payment produces a receipt that can be previewed through the simulated printer.

**Why this priority**: A complete business flow needs understandable navigation, recovery, and a
durable user-visible outcome after payment.

**Independent Test**: Traverse every route and payment outcome from a known cart. Confirm stack
behavior, reduced-motion behavior, retry safety, receipt values, and print-preview contents.

**Acceptance Scenarios**:

1. **Given** the operator advances from catalog to checkout, **When** back navigation is used,
   **Then** the previous screen and its state are restored predictably.
2. **Given** a payment is pending, **When** the operator tries to leave, **Then** Studio blocks or
   confirms the transition before checkout state can be abandoned.
3. **Given** a decline, timeout, or unavailable simulator result, **When** it is displayed, **Then**
   the operator receives a non-sensitive explanation and an appropriate retry or return action.
4. **Given** an approved result, **When** the receipt screen opens, **Then** merchant, lines,
   totals, result reference, and time agree with the confirmed checkout.
5. **Given** a completed receipt, **When** print is requested, **Then** a host-owned preview is
   created from structured receipt data without accepting device-control bytes from the plugin.
6. **Given** reduced motion is enabled, **When** routes or properties change, **Then** the same
   state transition completes without animated movement.

---

### User Story 5 - Build a Reactive Plugin Safely (Priority: P5)

A plugin developer defines screens from supported declarative components, binds displayed
properties to state and derived values, registers events, requests navigation and allowed host
actions, and packages the result for installation. Development errors produce actionable,
non-sensitive diagnostics.

**Why this priority**: Studio becomes a platform only when an external developer can build and
debug a useful plugin without depending on host internals.

**Independent Test**: Starting from the supported SDK example, create a small counter-and-total
screen, package it, run it in developer mode, and confirm state changes produce targeted visible
updates and invalid operations produce documented errors.

**Acceptance Scenarios**:

1. **Given** supported layout and control declarations, **When** the developer packages and opens
   the plugin, **Then** Studio presents an equivalent native hierarchy.
2. **Given** state, a derived value, and a bound text property, **When** state changes, **Then** the
   derived value recalculates once per batch and the bound property displays the result.
3. **Given** an unknown component, property, route, event, capability, or malformed update,
   **When** the plugin emits it, **Then** Studio rejects it with a stable error and does not partly
   mutate the active interface.
4. **Given** a plugin that traps or exceeds a resource budget, **When** Studio terminates it,
   **Then** the host remains responsive and presents a host-owned failure surface.
5. **Given** a terminated plugin and its host-owned failure surface, **When** the operator chooses
   restart, **Then** Studio creates a fresh isolated instance without restoring state from the
   terminated instance.

### Edge Cases

- The initial interface is empty, exceeds the node or depth budget, or contains duplicate IDs.
- An update targets a removed node, inserts at an invalid position, or mixes valid and invalid
  operations in one batch.
- A plugin sends invalid text encoding, invalid memory ranges, oversized messages, or messages in
  the wrong lifecycle state.
- A plugin loops forever, grows memory repeatedly, floods updates or actions, traps during an
  event, or attempts to use undeclared system functionality.
- Two plugin instances use identical local node, request, or route identifiers.
- A secret expires while a confirmation dialog is open or a plugin stops while a secret exists.
- The confirmed amount differs from the plugin's current cart after confirmation begins.
- A payment result arrives after the operator attempts navigation or after the plugin instance is
  replaced.
- Receipt printing is requested twice or before a successful payment.
- Native focus moves while a targeted update changes the focused control or one of its ancestors.
- The Wayland compositor disconnects while the plugin is running; Studio cancels pending actions,
  revokes secrets, terminates plugin instances, and exits without automatic session restoration.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Studio MUST verify a production bundle's integrity, publisher trust, identity,
  declared permissions, compatibility, and resource declarations before executing it.
- **FR-002**: Studio MUST reject malformed, altered, oversized, unsupported, or incompletely
  declared bundles without executing plugin logic.
- **FR-003**: Studio MUST provide an explicit developer mode for selected unsigned local bundles,
  visibly identify untrusted execution, and retain all non-signature protections.
- **FR-004**: Studio MUST run each plugin in an isolated instance with no direct filesystem,
  environment, network socket, device, process, or host-secret access.
- **FR-005**: Studio MUST expose only documented, version-compatible host interactions and MUST
  reject unknown or undeclared interactions.
- **FR-006**: Studio MUST enforce finite execution, memory, structure, message, and pending-request
  budgets so a faulty plugin cannot indefinitely block or exhaust the host.
- **FR-007**: Studio MUST display a host-owned error surface and remain usable after plugin load,
  validation, execution, or resource failures. For a plugin terminated by a trap or resource
  violation, this surface MUST offer a manual restart that creates a fresh isolated instance and
  MUST NOT restore state from the terminated instance.
- **FR-008**: A plugin MUST be able to declare an initial hierarchy using the supported layout,
  display, interaction, and overlay component catalog.
- **FR-009**: Studio MUST validate the complete initial hierarchy before displaying any of it.
- **FR-010**: A plugin MUST be able to update an individual property, insert a subtree, remove a
  subtree, or replace a subtree using stable node identifiers.
- **FR-011**: Studio MUST validate a complete update batch and either apply all of it or none of it.
- **FR-012**: Property-only updates MUST preserve unrelated native focus, scroll, input, and
  interaction state.
- **FR-013**: Studio MUST deliver non-secret native interaction events only to the plugin instance
  that owns the source control.
- **FR-014**: The supported SDK MUST provide state, derived value, effect, and batching behavior
  that emits only the changes caused by an event.
- **FR-015**: Derived values MUST update deterministically, effects MUST stop when their owning
  interface is removed, and reactive cycles MUST fail without freezing Studio.
- **FR-016**: Studio MUST provide nested and parameterized routes with push, replace, pop, pop-to,
  reset, lazy screen creation, bounded stack depth, and a not-found outcome.
- **FR-017**: Navigation requests MUST be validated by Studio before changing the active stack.
- **FR-018**: Studio MUST support predictable navigation and property transitions and MUST honor a
  reduced-motion preference without changing resulting state.
- **FR-019**: Sensitive input MUST be captured and retained exclusively by Studio-owned controls.
- **FR-020**: Plugins MUST receive only opaque references to sensitive input and MUST NOT be able
  to resolve, enumerate, transfer, or persist the underlying secret through Studio.
- **FR-021**: Sensitive references MUST expire, support single-use policies, and be scoped to the
  owning publisher, plugin, instance, purpose, and business session.
- **FR-022**: Studio MUST show a trusted confirmation containing publisher or merchant identity,
  exact amount, currency, and simulator status before accepting a payment action.
- **FR-023**: The payment action MUST use the confirmed amount and currency even if plugin state
  changes after confirmation begins.
- **FR-024**: The payment simulator MUST produce documented approved, declined, timeout, and
  unavailable outcomes without contacting a real provider.
- **FR-025**: Payment requests MUST be idempotent for the lifetime of the host process. Studio MUST
  retain at most 10,000 terminal payment records, MUST continue serving replays of retained keys,
  and MUST reject new unique payment requests rather than evict a retained record when full.
- **FR-026**: All monetary quantities MUST preserve exact integer minor-unit values and an explicit
  currency from cart calculation through receipt output.
- **FR-027**: An approved payment MUST produce a structured receipt whose merchant, lines, totals,
  time, and result reference agree with the confirmed checkout.
- **FR-028**: The printer simulator MUST accept only structured receipt information, record one
  previewable job per accepted request, and expose no raw device-control channel to plugins.
- **FR-029**: Studio MUST redact raw secrets and active sensitive references from logs, diagnostics,
  snapshots, receipts, persisted files, and action results.
- **FR-030**: Studio MUST support keyboard focus traversal and expose meaningful labels and states
  for interactive controls used by the reference checkout.
- **FR-031**: Studio MUST operate only in a native Wayland session and MUST clearly reject an
  unsupported display session without fallback. If the active compositor disconnects, Studio
  MUST cancel pending actions, revoke active sensitive references, terminate plugin instances,
  and exit cleanly without automatically restoring the session.
- **FR-032**: The reference plugin MUST demonstrate catalog, cart, checkout, protected input,
  deterministic payment, recovery, receipt, and print-preview behavior as one coherent flow.

### Key Entities

- **Plugin Bundle**: Installable unit containing identity, publisher, compatibility, permissions,
  resource declarations, executable content, assets, and integrity proof.
- **Plugin Principal**: The verified publisher, plugin identity, bundle identity, and running
  instance to which permissions and sensitive references are scoped.
- **Plugin Instance**: One isolated lifecycle containing state, interface ownership, resource
  budgets, pending actions, and current status. A manual restart creates a new instance identity
  and lifecycle with no state inherited from the terminated instance.
- **UI Tree and Node**: The validated retained interface hierarchy and each stable, typed element
  within it.
- **Update Batch**: An ordered, atomic set of property or structural changes for one instance.
- **Route and Navigation Stack**: Declared destinations, parameters, history, and active screen
  state owned by Studio for one plugin instance.
- **Capability Declaration**: The bounded set of host-mediated actions a bundle may request.
- **Action Request and Result**: A correlated asynchronous request for one allowed operation and
  its success or stable failure outcome.
- **Opaque Secret Reference**: A non-secret identifier for host-owned sensitive data, constrained
  by owner, purpose, session, lifetime, and use count.
- **Checkout Session**: Cart totals, currency, confirmation state, idempotency identity, payment
  state, and final receipt relationship for one sale attempt.
- **Receipt and Print Job**: Structured records of an approved simulated sale and its host-owned
  preview request.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An operator can launch the trusted reference plugin and see its first usable screen
  within 150 milliseconds during a warm start on the documented baseline device.
- **SC-002**: At least 95 percent of ordinary catalog, cart, and navigation interactions visibly
  respond within 100 milliseconds on the documented baseline device.
- **SC-003**: An operator can complete the reference catalog-to-receipt checkout in under two
  minutes without training beyond labels presented in the interface.
- **SC-004**: All approved, declined, timeout, and unavailable simulator scenarios produce the
  documented result and a recoverable next action in automated end-to-end tests.
- **SC-005**: One hundred percent of invalid-bundle and hostile-plugin fixtures are rejected,
  terminated, or contained without crashing or making the Studio shell unresponsive.
- **SC-006**: One hundred percent of secret-isolation tests find no raw sensitive value in any
  plugin-visible message, plugin memory fixture, log, diagnostic, snapshot, receipt, or persisted
  artifact.
- **SC-007**: Repeating any simulated payment request with the same idempotency identity produces
  exactly one recorded simulated transaction and the same result.
- **SC-008**: All reference checkout screens and actions can be completed using keyboard input,
  with focus order and accessible labels verified by automated and manual checks.
- **SC-009**: The distributed native executable contains no X11 or XCB runtime dependency and
  never opens through XWayland in supported acceptance environments.
- **SC-010**: An idle plugin with no pending event consumes no continuous plugin-driven rendering
  loop and produces no repeated interface messages.
- **SC-011**: A plugin developer can build, package, open, and diagnose the documented starter
  plugin in under ten minutes using only the repository guide and supported local tools.
- **SC-012**: Every milestone-one requirement is linked to at least one passing automated test or
  an explicitly documented manual acceptance check before release.

## Assumptions

- Milestone one serves a single local operator and one visible plugin instance at a time.
- Payment and printing are simulations; no real provider, terminal, printer, or external network
  is contacted.
- Production bundles are installed from a trusted local source; marketplace discovery and remote
  installation are outside this feature.
- Milestone-one production launch uses `--bundle` with an administrator-provisioned absolute path
  to one local `.studio` file. Studio does not infer trust from that path and still performs every
  production verification step.
- The host trust store is provisioned by the platform administrator before production bundle
  installation.
- Plugin developers use the supported SDK and packaging tools rather than constructing raw
  protocol messages as the normal development path.
- The reference catalog uses deterministic local sample data and requires no persistent business
  database.
- General persistent plugin storage, generic outbound requests, arbitrary files, clipboard,
  media, child processes, child plugins, and raw graphics access are outside this feature.
- Web, macOS, Windows, X11, and XWayland hosts are outside this feature.
- Multi-plugin screen composition, background services, and unattended payment processing are
  outside this feature.
- Touch-style means controls sized and behaved for tap and drag interaction through the milestone
  Wayland pointer path; direct multi-touch gesture recognition is outside this feature.
- Platform administration, publisher key issuance, key recovery, and legal payment compliance are
  future operational features; milestone one validates the technical trust boundary with local
  development keys and simulators.
