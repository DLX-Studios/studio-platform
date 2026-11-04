# Validation: Studio file routing

Date: 2026-11-03. Scope: FR-001–010, local tickets T001–T016.

## Implementation coverage

| Requirement | Implementation and evidence |
| --- | --- |
| FR-001/002 | Shared route discovery/validation, root/index/nested/parameter/catch-all precedence, layouts/private exclusions, confined symlinks and negative route registry tests. |
| FR-003 | Project composition, typed props, independent static instance state, nested imports/layouts, bounded IDs/imports and source-linked diagnostics. |
| FR-004/005 | Rust ProjectRouter owns history and admission; dev IR journeys and actual ASC/Wasm mounted through StudioHost verify navigation/state restoration. SDK matching uses the same positional rule. |
| FR-006 | Preview state/history/deadlines survive compatible source reloads; failed compilation/mount retains last valid surface. Imported sources retain actual source links. |
| FR-007 | Legacy TypeScript route and single-entry source regressions retained. Explicit declared-screen preview projects retain precedence. |
| FR-008 | Runnable stream example, authoring guide, feature contract and local task list. |
| FR-009 | Bounded signed timer metadata, pure manual-clock scheduler, sequential status rechecks, native IR and Wasm timer delivery, completion delay/reset/route cancellation. |
| FR-010 | Closed presentation props, retained native opacity tracks, native full-viewport cover bounds, large-text metrics/zoom and working foreground input. Fixed outlined title art and local sunset background. |

## Focused verification

Cargo uses the shared target directory, one active Cargo process, and consistent
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.
This machine has the runtime xkbcommon-X11 library but lacks its development
link name; `LIBRARY_PATH=/tmp/studio-routing-link-lib` points to a temporary
symlink to the installed real library. No repository dependency or stub library
was introduced for this environment workaround.

Passed checks recorded during implementation:

- Native renderer: seven tests covering cover background bounds/foreground input,
  large text and zoom, timed opacity with retained identity, and reduced motion.
- Runtime library with development feature: fourteen tests covering admission,
  denied/malicious remounts, structural input targets, hidden timer focus,
  sequential timers, delayed completion, catch-up and route cancellation.
- CLI library: forty-three tests; route registry: eight tests; twelve project integration tests (including the actual example).
- Script composition: seventeen tests; routed Wasm emission: eleven tests.
- Shared navigation tests: root/parameter/catch-all resolution, bounded history,
  signed metadata, ten timer scheduler tests and transition regressions.
- SDK navigation: nine Bun tests, 77 assertions; SDK `bun run check` passed.
- Focused native/runtime Clippy with all targets passed.
- Actual composed Timer source compiled by ASC and mounted through StudioHost:
  no early calls, tick/fade/status completion, delayed live navigation and hidden
  internal callbacks verified.

## First-failure evidence

Behavioral regressions were run before their implementation: unsupported root
and wildcard routes, missing project imports/layout composition, missing host
navigation, timer metadata/scheduling, common-property detection, opacity
patching/interpolation, large-text/background rendering, shallow conditional
and object AST lowering, scalar string emission, invisible focus targets,
cross-timer reset ordering, standalone imported source maps, reserved metadata
collisions, linked route discovery, composed ID limits and typed derived props.
The full example is additionally compiled and mounted as a regression fixture.

## Integration checkpoint

- `cargo fmt --all -- --check`: passed before the workspace test gate.
- `studio check examples/stream-countdown`: three discovered routes passed.
- `studio build examples/stream-countdown`: succeeded; archive inspection confirmed
  all three route patterns, timer/source-map metadata and all three SVG assets.
- Actual full example source compiled by ASC and admitted by StudioHost: initial
  `10:00`, live title artwork selection, and back navigation/state restoration passed.
  Its richer composed page exceeded the scaffold's one-million event-fuel budget;
  the example now declares the existing standard ten-million budget. Host limits
  and admission rules were not increased.
- Native Wayland preview on port 5128: ready at revision 1, 1440 × 900, three
  available screens. An invalid imported-source edit reported its real source
  while retaining revision 1; valid restoration/whitespace edits committed through
  revision 4 with the original source hash restored. The smoke process was stopped;
  the user's existing starter preview was left running. The compositor does not
  expose screencopy, so no screenshot review is claimed. Native renderer tests
  provide geometry, input, text metrics and timed opacity evidence.
- `cargo test --locked --workspace`: passed, 1,478 tests; two existing ignored tests
  (`real_package_resolves_byte_identical_svgs` and explicit STUDIO-BENCH-1 qualification).
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
  The first pass found an unnecessary raw-string delimiter in a new test;
  removing it preserved the exact string value.
- Final `cargo fmt --all -- --check` and `git diff --check`: passed.
- Two consecutive builds of the restored example produced byte-identical signed
  archives (SHA-256 comparison).

## Acceptance boundaries

This feature implements native Studio source routing; it does not introduce
browser HTML, CSS, SSR, server loaders, SvelteKit HTTP endpoints, npm execution,
or custom cross-component event forwarding. Layout state is per compiled page;
stateful imported components in keyed collections are rejected explicitly.
The example authors a 1440 × 900 desktop presentation with adjustable header
width and native preview zoom. Automatic responsive breakpoints are not claimed.
No working font is bundled: two fixed title artworks contain outlined Pricedown
lettering with attribution in the example README.

Publisher trust and per-platform release certification remain separate from
these development/compiler tests. Unrelated Designer tickets 44/68/69 retain
their broader acceptance criteria. No release-profile build or commit/push was
requested or performed.

## Final consistency and convergence review

FR-001–010 map to implemented source and executed acceptance evidence; all sixteen
feature tasks are complete. Spec, plan and tasks agree on route ownership, source
composition, bounded metadata, timers and native presentation. No remaining
buildable requirement within this feature scope was found, so convergence adds
no remediation tasks. The acceptance boundaries above remain explicit.
