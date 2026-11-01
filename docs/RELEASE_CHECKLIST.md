# Milestone-One Release Checklist

Checked items below retain their original milestone evidence; they do not certify the
2026-10-31 dependency upgrade. Re-run target-platform release gates for a new release.

## Current product completion gate — 2026-11-01

The historical automated checks below do not establish a complete native Designer journey.
The [completion audit](development/PROJECT_COMPLETION_AUDIT.md) found ownership/isolation,
native canvas parity, build/sign/launch and other product-wiring gaps. Resolve the ordered
[completion backlog](../.scratch/studio-designer/completion-tasks.md) and run the
[Designer acceptance journey](development/DESIGNER_ACCEPTANCE_JOURNEY.md) before claiming
Designer v1 release readiness. Current baseline: `e21ed734`; native Windows/macOS reload/CLI
CI passes, while full applications/installers and manual release acceptance remain separate gates.
The original Linux CI job exhausted runner disk space (C021). The profile/storage fix completed
Rust tests and Clippy with 12 GB of artifacts and 67 GB free; a stale display-message assertion
was then corrected and passed the local headless Wayland test. A fresh green remote run remains
required. Current local integration evidence: 1,293 workspace tests passed, two explicit
qualification tests ignored, and workspace Clippy passed.

## Automated gates

- [x] Ticket 59 flagship deterministic evidence report: `cargo run --locked -p studio-flagship --bin flagship-evidence`
  (the report is expected to remain `release_ready: false` until its explicit hardware, cloud, and
  prerequisite integration gaps are closed; the 2026-09-03 run passed all deterministic gates and
  reported those blockers; see [Flagship release evidence](release/FLAGSHIP_RELEASE_EVIDENCE.md)).

- [x] `bun run test:all`
- [x] `./scripts/test-starter-quickstart.sh`
- [x] four 60-second fuzz smoke runs
- [x] `./scripts/benchmark-acceptance.sh` (informative host; low-power baseline intentionally deferred)
- [x] release build with `DISPLAY` unset
- [x] `./scripts/check-platform-features.sh` (2026-10-31: GPUI Kit 0.8.1 / GPUI 0.3.10; replaces old no-X11 gates)
- [x] headless native Wayland launch with no new XWayland process
- [x] `bun test tests/traceability.test.ts`

## Manual native gates

- [ ] Complete and record every check in `docs/accessibility/ACCEPTANCE.md`.
- [ ] Complete catalog-to-receipt with keyboard only in under two minutes.
- [ ] Confirm trusted input confirmation failure and preview surfaces are visually distinguishable.

## Security and provenance

- [ ] Review `docs/security/THREAT_MODEL.md` residual risks.
- [ ] Review the Cargo/Bun lockfile delta and vulnerability advisories; see
  `docs/security/RELEASE_REVIEW_2026-08-04.md` (known-vulnerability scan is clean; unmaintained
  transitive crates remain for review).
- [ ] Confirm Oxide extraction ledger and published GPUI Kit versions/notices remain accurate.
- [ ] Obtain legal confirmation for licenses of the final linked GPUI binary and collect notices;
  the release review identifies GPL-linked Zed crates and metadata gaps requiring confirmation.
- [ ] Confirm no production provider terminal printer network or filesystem capability was added.

## Release decision

- [x] Attach dated validation reports with hardware commands, timings, and results:
  [secure runtime report](../specs/001-secure-plugin-runtime/validation-report.md) and
  [component-platform report](../specs/002-component-platform/validation-report.md).
- [ ] Record manual tester and any approved waiver.
- [ ] Sign off engineering security accessibility legal and release ownership.
- [ ] Publish only when every non-waived item above is checked.

## Current waivers and deferrals

- The formal `STUDIO-BENCH-1` N100/Weston run is deferred by the current release decision. This
  does not claim performance certification for low-cost POS hardware.
- Hardware-specific performance certification is outside the current milestone sign-off scope;
  the automated benchmark remains recorded as informative evidence.

## Current verification note

The 2026-09-03 integration checkpoint passed `cargo fmt --all -- --check`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`, and
`cargo test --locked --workspace`. The JavaScript gates were re-certified the same day with the
pinned Bun 1.3.9: `bun install --frozen-lockfile` (1249 packages, `bun.lock` unchanged),
`bun run check`, root `bun test` (38 pass; `tests/platform/wayland_startup.test.ts` requires the
headless Sway harness, which this host does not have installed), the `sdk/assemblyscript` suites
(15 pass, including `component-catalog.test.ts`), and a clean generated-artifact drift check.
The earlier "frozen install rejects the lockfile" failure came from a local Bun
`1.4.0-canary.1`, not from the checkout filesystem. No lockfile or package manifest changes were
made.

A second 2026-09-03 checkpoint closed the flagship production-seam tickets: 24, 25, 27, 29, 30,
49, 51, and 53 are integrated with verified seams and tests, ticket 34 stays explicitly scoped to
the physical baseline run, and the flagship `prerequisites` verification gap now derives from the
actual prerequisite statuses (see [Flagship release evidence](release/FLAGSHIP_RELEASE_EVIDENCE.md)).
That checkpoint passed `cargo fmt --all -- --check`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`, and
`cargo test --locked --workspace` from a fresh per-commit cargo cache after the bloated
repository-local target directory (which had filled the checkout filesystem and broke linking)
was removed.
