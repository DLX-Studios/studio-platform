# Feature Specification: Structural Patches

**Feature Branch**: `014-structural-patches`

**Created**: 2026-09-06

**Status**: Draft

**Input**: `.studio` handlers patch text today; lists cannot change
length. Worse, `{#each}` ignores its key (`key: _`) and stamps every
row with identical positional ids, so multi-row lists fail mount with
duplicate identities. Dynamic apps (cart lines, feeds, results) need
keyed rows plus insert/remove/replace operations from both legs.

## Clarifications

### Session 2026-09-06

- Q: How do rows get identities? → A: Every node in a row subtree is
  suffixed with the row key: `{id}#{key}`. The key expression evaluates
  per item against item/index locals; non-string keys stringify like
  text content. Duplicate keys in one collection are a check-time
  error (stable code); missing keys fall back to positional index
  with the same suffix scheme.
- Q: How do authors mutate lists? → A: Array-slot statements:
  `items.push(expr)` appends one record, `items.pop()` drops the
  last, `items.remove(index)` drops one position,
  `items.clear()` empties. Anything else on arrays is a check-time
  diagnostic. Records appended must carry exactly the statically
  known field set (extra/missing fields are check-time errors).
- Q: How does the compiled guest store lists? → A: Parallel scalar
  arrays per statically used field (`state_items_name: Array<string>`),
  collected from the template's `item.<field>` reads. Push/pop/remove
  splice every array; row builders index them. Non-scalar or
  dynamically-named fields fail at build with a diagnostic, never
  silently. Array state needs shape evidence (non-empty literal
  initial elements or record-literal pushes, all sharing one exact
  scalar field set); without it, item reads are check-time errors.
- Q: What can list element shapes be? → A: Flat records of
  String/Number/Boolean only. Dynamic row bodies must be static
  structure (components/text/interpolations; no nested `If`/`Each`,
  whose counts would break position mapping). Fallbacks follow the
  same rule and must not read item/index locals.
- Q: Which ops? → A: `ReplaceNode` on the row-list wrapper, never
  surgical insert/remove. Per event, for every list whose slot was
  mutated, the guest rebuilds that wrapper's children wholesale and
  emits one `ReplaceNode`; content inside rides along fresh, so no
  separate content ops are needed for rows. Reordering, appends, and
  removals all collapse to wholesale replace — no move detection, no
  index arithmetic, no position-dependent divergence between legs.
  `InsertChild`/`RemoveNode` stay unused in v1.
- Q: What about `{#if}` branches? → A: Branch take/drop is out of
  scope, and additionally: content inside `If` subtrees and inside
  static (never-mutated) `Each` subtrees is never patched by either
  leg — patches address static trees and rebuilt dynamic rows only.
  Dynamic row bodies must be static structure (no nested `If`/`Each`);
  the validator rejects anything else at check time.
- Q: What about `{#if}` branches? → A: Branch take/drop is out of
  scope (still mounts, never patches). Structural ops cover keyed
  rows only.
- Q: Sequence numbers? → A: Unchanged: one counter per session/call
  stream, strictly increasing, shared with content patches.

## User Scenarios & Testing

### User Story 1 - Cart lines appear (Priority: P1)

A shopper taps Add: a new cart-line row (keyed by product id) inserts
under the order list with name, quantity, and line total; tapping the
row's trash removes it. Both legs emit identical op sequences;
patches apply to a mounted `UiRegistry` in tests and update the live
window in the manual pass.

### User Story 2 - Duplicate keys fail at check (Priority: P1)

A static collection literal with two identical keys fails `studio
check` naming the key. Positional fallback (no key expression) keeps
working for static single-render lists.

## Functional Requirements

- FR-1: Validator extracts array mutations with field-set checking;
  key duplication in static collections is a check-time error.
- FR-2: Lowering records row plans (key expr, field set, body) per
  `{#each}`; row subtree ids suffix with keys in both legs.
- FR-3: Evaluator rebuilds mutated lists wholesale: row subtree ids
  suffix with keys, and each mutated list yields one `ReplaceNode`
  with rebuilt children. Content ops skip branch/item subtrees on
  both legs, so row content updates exclusively through replacement.
- FR-4: Backend generates parallel-array state, row builders (one
  function per template node, parameterized by item index), splice
  statements, and `ReplaceNode` envelopes in template order.
- FR-5: Differential harness drives multi-event list scripts
  (append/append/remove/replace-by-key-change) asserting byte parity
  plus `UiRegistry` application of every emitted patch.
- FR-6: Existing goldens re-bless for suffixed row ids; single-row
  fixtures keep working unchanged.

## Non-Goals

- Branch take/drop patches.
- Reordering moves (remove+insert pairs cover it).
- Non-scalar item fields in the compiled leg.
- Check-time value validation (separate ticket).

## Success Criteria

- SC-1: `cargo test -p studio-script` green with list fixtures.
- SC-2: Manual pass: cart lines insert/remove live in `studio dev`.
- SC-3: Full workspace gate green.
