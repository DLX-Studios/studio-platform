# Feature Specification: Stateful Studio Script Interactions

**Feature Branch**: `009-studio-interactions`

**Created**: 2026-09-05

**Status**: Draft

**Input**: Compiled `.studio` guests are static: handler `$state` mutations compile to
nothing, `studio_event` returns fixed initial-value emissions of type `event`, and the
native host rejects non-`patch` event-call emissions. Buttons in modern-script apps
therefore cannot update the screen. This feature closes the loop: handlers mutate
runtime state, the guest re-evaluates affected bindings, and `studio_event` returns a
`patch` envelope the host applies. No host changes required.

## Clarifications

### Session 2026-09-05

- Q: What happens to the `emit()`-exactly-once handler rule? → A: Relaxed to at most
  once. A handler with `emit()` keeps designer-channel semantics (mutations apply first,
  then the payload evaluates against updated state). A handler without `emit()` produces
  a `patch` envelope. Emit and patch are never combined: one guest call returns exactly
  one message.
- Q: Which state mutations are supported? → A: Assignment and compound assignment
  (`=`, `+=`, `-=`) plus `++`/`--` on `$state` Number/String/Boolean slots. Anything
  else is a check-time diagnostic, not silent dead code.
- Q: Which UI updates do patches cover? → A: `update_prop` on `text`/`label` content
  props whose value expressions reference (transitively, through `$derived`) a mutated
  binding. Structural ops (insert/remove/replace) are out of scope.
- Q: Who owns patch sequence numbers? → A: The guest. First patch is sequence 1,
  strictly increasing per call, matching the host's existing
  `validate_patch_sequence` contract.
- Q: Does the static mount change? → A: No. Initial values stay baked; only event
  calls become dynamic. The backend doc comment claiming "no state-mutation
  operations" is updated, and the IR version stays 2 (additive capability within
  the same IR, recorded in the feature report).

## User Scenarios & Testing

### User Story 1 - Tap Add to Cart, see the bag update (Priority: P1)

A shopper taps "Add" on a product card. The guest applies the handler mutations,
recomputes derived totals, and returns `{"type":"patch","payload":{"sequence":N,...}}`
with `update_prop` ops for the cart count and total text nodes. The host applies the
patch; the bag count and total change without a reload.

Acceptance: scripted `pressed` events against the compiled wasm produce byte-parity
patch sequences with the evaluator leg (differential harness), and the emitted
patches validate under `decode_guest_message` + apply cleanly to the mounted tree
in `UiRegistry`.

### User Story 2 - Handlers without emit are first-class (Priority: P1)

An author writes `function addTee() { cartCount += 1; }` with no `emit()`. `studio
check` accepts it; `studio build` succeeds; pressing the button patches the screen.
An author writing an unsupported mutation (e.g. mutating a `$derived` slot, or
calling an unknown function) gets a check-time diagnostic naming the handler.

### User Story 3 - Designer events keep working (Priority: P2)

A handler with exactly one `emit()` keeps today's designer-channel behavior, except
the payload now evaluates against post-mutation state. Existing fixtures and their
goldens are unchanged (all fixture handlers emit exactly once with no mutations).

## Functional Requirements

- FR-1: The validator extracts per-handler state mutations (`Mutation { slot, op,
  operand }`) into the IR; unsupported mutation targets/operations are
  check-time errors (stable codes, no silent drops).
- FR-2: The emit-at-most-once rule replaces emit-exactly-once; zero-emit handlers
  are valid and produce patches.
- FR-3: The evaluator applies mutations to a mutable scope, recomputes derived
  slots, re-projects affected content props, and returns a `PatchBatch`
  (sequence tracked per scope session, starting at 1).
- FR-4: The ASC backend generates runtime state (wasm globals), per-handler
  mutation + recompute + patch-JSON code, and a sequence counter; `studio_event`
  returns the patch envelope for zero-emit handlers and the (fresh-valued)
  event emission for emit-handlers.
- FR-5: Affected-node analysis is conservative and shared: a content prop
  (`text`/`label` on `Text`/`Button`, folded or direct) is patched when its
  value expression references a mutated binding directly or through `$derived`.
- FR-6: The differential harness drives scripted event sequences through both
  legs (eval scope vs compiled wasm) and asserts identical message sequences;
  new interaction fixtures cover count, derived total, and multi-handler order.
- FR-7: `studio check` reports unsupported mutations with handler names; `studio
  build` succeeds for the interaction fixtures and the patches they emit
  validate under the protocol.

## Non-Goals

- Structural patches (insert/remove/replace) from `.studio` handlers.
- String-concatenation or non-arithmetic expression statements beyond the
  mutation ops above.
- Changes to the native host: it already applies patches (pos-desktop proves it).
- Check-time child-shape validation (separate ticket; mount errors name nodes).

## Success Criteria

- SC-1: `cargo test -p studio-script` green, including new interaction fixtures
  with scripted multi-event sequences identical across eval and wasm legs.
- SC-2: A `.studio` counter app increments its displayed count on every press
  through `studio dev` against the real host (manual pass).
- SC-3: Full workspace gate green (fmt, clippy `-D warnings`, 1173+ tests).
