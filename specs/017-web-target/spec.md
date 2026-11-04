# Web compilation and hosted stream countdown

Status: Implemented, verified and hosted. User request: make the existing Studio countdown compile to web and host it.

## Stories and requirements

US1/P1: Build the existing .studio sources as a static browser app.
- FR-001: `studio build <project> --target web` compiles the same routed .studio source to Wasm and exports a self-contained static directory, preserving native build behavior.
- FR-002: A closed DOM adapter supports the example's Box, Scaffold, Column, Row, Text, Image, Button and Tag nodes. Unsupported kinds fail the web build with a diagnostic. Props/assets use the existing protocol contract and manifest allowlist.

US2/P1: Use the existing stream behavior in a browser.
- FR-003: Compile studio-navigation's ProjectRouter and TimerSchedule to a small Rust Wasm bridge; reuse matching, history and monotonic catch-up/completion rules rather than writing another router.
- FR-004: Execute guest Wasm in a dedicated Worker with closed imports, checked memory/message bounds, owner-authorized mounts and input handlers; terminate a stuck worker after a bounded request deadline. No native capabilities, WASI, secrets, arbitrary plugin upload or native bundle trust claims.
- FR-005: Retain element identity during property updates, reject unsafe nodes/URLs/properties and unsupported actions, preserve page state on navigation, and align browser history including direct links and browser Back.
- FR-006: Show the viewport-cover artwork and readable responsive title/clock; support the existing pulse, fade and delayed /live transition, including reduced motion and keyboard-accessible buttons. Start at600 seconds on fresh load and support Start now/Reset/Go live/Back.

US3/P1: Publish a working version.
- FR-007: Validate the actual example in a real browser at desktop/mobile sizes and exercise ticks, reset, delayed exit, navigation/history and asset loading. Document the supported web subset.
- FR-008: Publish only the prepared static app and its source snapshot through Sites, preserve the default private audience, persist the exact project identity and deployment URL, and report a verified deployment result.

## Clarifications resolved from context

The existing .studio sources remain authoritative. This is an explicit web target for authored static apps, not an extension of native signed-bundle admission to the browser. Browser execution uses a Worker deadline rather than claiming Wasmtime fuel/epoch parity; the native runtime remains its own certified boundary. No new privileged capability is exposed. Sites creates an owner-private site by default; user authorization to host covers publication. The site source export must exclude the platform's unrelated dirty files and credentials.

## Acceptance

Build succeeds with routes/layouts/imports and a local HTTP server; the actual clock begins10:00, ticks, fades before /live, resets pending exit and retains navigation state. Browser Back returns to the correct page. Deep links render. Invalid metadata/messages/routes/assets fail closed. Native regressions pass. A successful Sites deployment result supplies the live URL.
