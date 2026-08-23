# Component Platform Research

## Decision 1: Use gpui-component as the native source

**Decision**: Prefer the synchronized gpui-component fork for controls, overlays, feedback,
navigation, lists, data display, and themes.

**Rationale**: It is already integrated with Studio's GPUI revision and Wayland-only feature policy.
It avoids a second runtime UI library and keeps native behavior consistent.

**Alternatives considered**: adabraka-ui remains a design and animation reference; importing it
would add a second GPUI ecosystem and duplicate theme/event abstractions.

## Decision 2: Keep a Flutter-inspired guest vocabulary

**Decision**: Use Flutter-like names and composition semantics in the closed protocol, mapping them
to GPUI primitives or gpui-component implementations.

**Rationale**: Plugin authors receive a declarative, familiar hierarchy without embedding Flutter,
HTML, CSS, or a browser.

**Alternatives considered**: Exposing Rust library names directly would couple guest contracts to
upstream implementation details.

## Decision 3: Add missing patterns as Studio compositions

**Decision**: Implement selected Card, Empty, Field, ListTile, Scaffold, Banner, and related patterns
as Studio-owned compositions when no suitable gpui-component exists.

**Rationale**: These patterns are small semantic compositions and can reuse existing native
primitives while preserving one protocol and accessibility model.

**Alternatives considered**: Adding adabraka-ui as a dependency would increase binary, license,
feature, and compatibility surface.

## Decision 4: Add by independently testable batches

**Decision**: Deliver display/feedback, forms, overlays, navigation, and data batches in priority
order, with protocol, native, SDK, and POS tests in each batch.

**Rationale**: The catalog is broad; vertical slices prevent an unreviewable bulk implementation and
keep failures localized.

## Decision 5: Preserve security and platform gates

**Decision**: Every component update must pass closed-schema validation, ownership checks,
accessibility/reduced-motion tests, and no-X11 feature/linkage checks.

**Rationale**: UI expansion must not weaken host authority, secret isolation, or the Wayland-only
runtime guarantee.
