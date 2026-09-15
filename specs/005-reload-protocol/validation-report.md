# Validation Report: Runtime Reload and Preview Protocol

**Validation date:** 2026-09-03
**Implementation result:** T001–T015 complete (T016 full gate deferred to the
post-008 verification phase per directive). The CLI-to-runtime reload channel,
atomic prepare/commit swap semantics, session control, and state policy are
implemented with scripted, storm, recovery, and end-to-end coverage.

## Automated evidence (deferred-execution suites)

| Suite | Coverage |
| --- | --- |
| `studio-protocol` reload unit tests | Message encode/decode round-trips, correlation ids, protocol-mismatch and unknown-type rejections |
| `studio-host` reload unit tests | Supersede keeps one trailing reload; revision advances exactly once per commit |
| `crates/studio-host/tests/reload_session.rs` | 10 scripted reloads (7 valid, 2 invalid, 1 crashing) end on the last valid surface with revisions 2–8 and zero dead windows |
| `crates/studio-host/tests/reload_storm.rs` | Storm collapses to one trailing reload; mid-swap failure rolls back; first-bundle failure yields safe empty state; channel revision accounting |
| `crates/studio-cli/tests/reload_e2e.rs` | Real binary build plus real client path: reload accepted, missing bundle rejected with surface intact, restart re-instantiates, restart-without-live fails structurally, contract shape assertions |
| `studio-app` reload unit tests | Route preserved-if-declared-else-default; first mount lands on mount route; launch failures map to validate/instantiate/commit stages |

## Design notes

- Wire protocol lives in `studio-protocol::reload` so the CLI speaks it without
  the host's storage dependencies; the swap planner stays in `studio-host`.
- The `studio-app` server prepares through the existing admission path without
  mounting, queues accepted surfaces in the frame-loop inbox, and rolls back by
  dropping; the window drains the inbox per poll cycle without recreating
  entities.
- Route policy resolves against CLI-supplied declared routes; the shared route
  tracker starts empty (mount-route default) with a defined feed seam for
  navigation-owning shells.
- Dev trust semantics unchanged; production admission untouched.

## Scope and limitations

Display-level swap verification (real compositor, same-window observation)
belongs to the headless gate pattern with the documented scenario. Full
workspace gates run in the post-008 verification phase.
