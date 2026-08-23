# Milestone-One Release Checklist

## Automated gates

- [x] `bun run test:all`
- [x] `./scripts/test-starter-quickstart.sh`
- [x] four 60-second fuzz smoke runs
- [x] `./scripts/benchmark-acceptance.sh` (informative host; low-power baseline intentionally deferred)
- [x] release build with `DISPLAY` unset
- [x] `./scripts/check-no-x11-features.sh`
- [x] `./scripts/check-no-x11.sh target/release/studio-app`
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
- [ ] Confirm Oxide extraction ledger and gpui-component SHA/delta remain accurate.
- [ ] Obtain legal confirmation for licenses of the final linked GPUI binary and collect notices;
  the release review identifies GPL-linked Zed crates and metadata gaps requiring confirmation.
- [ ] Confirm no production provider terminal printer network or filesystem capability was added.

## Release decision

- [x] Attach `validation-report.md` with hardware commands timings and dated results.
- [ ] Record manual tester and any approved waiver.
- [ ] Sign off engineering security accessibility legal and release ownership.
- [ ] Publish only when every non-waived item above is checked.

## Current waivers and deferrals

- The formal `STUDIO-BENCH-1` N100/Weston run is deferred by the current release decision. This
  does not claim performance certification for low-cost POS hardware.
- Hardware-specific performance certification is outside the current milestone sign-off scope;
  the automated benchmark remains recorded as informative evidence.
