# Validation Report: File-Based Routing

**Validation date:** 2026-09-03
**Implementation result:** T001–T011 complete (T012 full gate deferred to the
post-008 verification phase per directive). `routes/` files map to a validated,
deterministic registry with a pure matcher shared across Rust tooling, the
guest SDK, and the reload route policy.

## Automated evidence (deferred-execution suites)

| Suite | Coverage |
| --- | --- |
| `route_registry` matrix | Static/nested/param/wildcard mapping, underscore exclusion, titles/defaults, duplicate/ambiguous/malformed/mismatch rejections naming files |
| `route_registry` golden | Render byte-stable; legacy `route_<name>` + `declaredRoutes` lines byte-identical to the previous scanner on the real pos-desktop project |
| `route_registry` battery | 12-path battery per the shared matching rule, plus explicit-miss without catch-all |
| `navigation-routes.test.ts` | Same battery shape in the guest SDK (`matchRoute`, params, wildcard, explicit miss) |
| `studio-app` reload tests | Parameterized preservation by pattern, reset fallback, launch-stage mapping |

## Design notes

- File path is the single source of truth; `route`/`title` string exports are
  cross-checked (mismatch diagnostic, path wins), never executed.
- The generated module keeps legacy exports byte-identical and appends the
  sorted `routeTable` plus `notFoundRoute`.
- Matching rule stated once (normalize, segment-pair, statics→params→wildcard,
  miss falls to not-found) and implemented three times with cross-checked
  batteries.

## Scope and limitations

Route metadata is titles only; richer metadata belongs to future work, not new
surface in this feature. Full workspace gates run in the post-008 phase.
