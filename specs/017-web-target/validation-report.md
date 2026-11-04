# Web target validation

Date: 2026-11-03. User request: compile the existing Studio countdown for web and host it.

## Evidence

- First-failure CLI tests rejected unknown --target; implementation now passes four
  tests for static export, unsupported kinds and unsupported properties, preserving
  the previous output on rejected sources.
- First-failure Rust bridge tests exercised eight unimplemented boundary cases;
  all eight now pass using the shared Rust navigation, timer and retained UI code.
- Browser-target protocol cross-check first failed on native reload socket types;
  gating native transports to Unix/Windows made the same protocol compile for Wasm.
- `scripts/build-web-core.ts` builds the actual wasm32-unknown-unknown host bridge
  and wasm-bindgen 0.2.127 bindings; generated Wasm is approximately 2.3 MiB.
- Nineteen Bun renderer/worker/watchdog tests pass (85 assertions), covering safe
  assets/text, retained element/focus identity, hidden inputs, checked guest bytes,
  imports/memory/queue bounds, host-authorized mounts, timers and rollback.
- Real browser testing first exposed Buttons incorrectly disabled when optional
  on_pressed was absent. A new test reproduced this, and the DOM adapter now uses
  the native default Button pressed contract correctly.
- Strict TypeScript checks pass for the browser SDK and root scripts.
- `cargo fmt --all -- --check`: passed.
- `cargo test --locked --workspace`: 1,489 passed, 0 failed, 2 existing ignored.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `git diff --check`: passed.

## Actual browser acceptance

Chromium 1248, controlled with Playwright 1.64.1, executed the real compiled app
through a local HTTP server at 1440×900, 390×844 and 320×844. Desktop background
bounds were exactly 1440×900; mobile pages had no horizontal overflow and retained
readable large clock metrics. Screenshots were inspected. The same source counted
from 10:00 to 9:59, reset, canceled a pending exit, interpolated its opacity before
the delayed /live route, returned with retained state, handled browser Back/Forward,
opened /live directly and used its catch-all page. Reduced motion used zero-duration
visual transitions. No page or loading errors were observed.

Command: `STUDIO_WEB_BROWSER=1 STUDIO_BROWSER_PATH=<existing-chromium> bun test ./sdk/web/tests/browser.acceptance.test.ts`.
Screenshots are local generated artifacts under `.scratch/web-qa/`.

## Isolation and supported scope

This is a static authored-app browser target. Guest code executes in a dedicated
Worker with a parent deadline (2 s active / 10 s startup), bounded Wasm memory,
closed imports and checked messages. Rust protocol validation, retained registry,
route ownership and shared timer scheduling remain authoritative. Browser controls
are distinct from native Wasmtime fuel/epoch metering and signed-bundle admission;
no such parity or privileged native capability is claimed.

The eight supported DOM kinds and exact property names are recorded in
sdk/web/contract.json, shared by the renderer and CLI validation. Native source
builds remain available; existing regression gates passed. No Cargo release-profile
build or platform-repository commit/push was performed.

## Publication

Site identity: appgprj_6aea46cdc8108191aefd8ef7d101500d.
Separate source checkout: `.scratch/sites/vice-city-stream-studio`.
The prepared static export and original authored source snapshot are published
through Sites with its default owner-private audience. Unrelated platform edits,
credentials, native build caches and tool binaries are excluded.
Publication succeeded through Sites:
https://vice-city-stream-studio.activematch.chatgpt.site

- Deployment: appgdep_6aea4e32bc80819199bc112cdf65a96d
- Saved version: appgprj_6aea46cdc8108191aefd8ef7d101500d~appgver_54b316d605308191bc63f7261d3e2f42
- Pushed source commit: 11f93f6c14a0308a8bcebfda94b6deb77af24fe9
- Static files: 16, approximately 2.5 MB uncompressed; deployment archive 419 KiB.
- Terminal native deployment status: succeeded, verified 2026-11-03T21:12:24Z.

The example's web-hosting.json persists these exact identifiers. The platform
workspace remains uncommitted; the isolated Site snapshot was committed/pushed
by the Sites source workflow as required for publication.

## Final hardening and consistency review

A focused post-checkpoint export test found standalone app.studio used /app while
its browser entry URL was /. The web export now declares / for a standalone
entry, verified by a first-failure regression and the final four CLI export tests.
This does not alter native routing or the countdown's existing file routes.

Static hosts may append a trailing slash to a directory route. A Worker regression
first observed /live/ reaching the core unchanged; startup now canonicalizes that
browser URL to /live before invoking the same strict Rust resolver. The nineteen
unit tests, strict TypeScript and actual browser journey were rerun successfully,
including direct /live/ loading. Final focused CLI Clippy and format checks passed.

FR-001–008 have implementation and acceptance evidence. Spec/plan/tasks agree on
source ownership, shared Rust routing/timers, the closed browser adapter, explicit
browser isolation controls and private publication. No remaining buildable gap
within this feature scope was found.
