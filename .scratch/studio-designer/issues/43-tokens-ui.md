# 43 [D]: Design tokens UI

**What to build:** Create, edit, apply, and override color/typography/spacing/radius/border/shadow/motion tokens. Inspector distinguishes shared intent from local values. Renames propagate while identities stay stable.

**Blocked by:** 38, 39

**Status:** partial — token models and Focus actions implemented; full native value-editing/render parity acceptance remains open

- [x] Apply→override→clear flow works per property — `crates/studio-design/tests/tokens.rs:146-188` and `crates/studio-designer/src/focus_view.rs:744-832` exercise the session-backed command path.
- [x] Rename propagates to all consumers without identity churn — `crates/studio-design/tests/tokens.rs:275-330` verifies the stable `TokenId`, renamed value, and usage query; native wiring is `crates/studio-designer/src/focus_view.rs:827-854`.
- [x] Deleting a referenced token requires confirmation listing usages — domain rejection/confirmation is covered at `crates/studio-design/tests/tokens.rs:332-374`; the native two-step usage listing and confirm action are at `crates/studio-designer/src/focus_view.rs:1440-1448` and `2557-2585`.
- [x] Token use shown for every inspected value — `DesignerQuery::NodeTokenValues` is rendered as shared/local provenance at `crates/studio-designer/src/focus_view.rs:2420-2468`; query semantics are covered at `crates/studio-design/tests/tokens.rs:190-207` and `225-271`.

The native browser also exposes a minimal shared-value edit action (`UpdateToken`) at `crates/studio-designer/src/focus_view.rs:858-888` and `2537-2543`, covering the ticket's create/edit wording. Token kinds remain domain-validated by the existing command engine.

## Completion audit — 2026-11-01

Current source/evidence assessment: F02/F08 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
