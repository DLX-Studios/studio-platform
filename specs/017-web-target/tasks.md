# Web target tasks

- [X] T001 Define FR001–008, browser boundary clarification, shared API, plan, research and acceptance in specs/017-web-target (all FRs).
- [X] T002 [P] Add first-failure shared WebHost route/timer/protocol/negative tests in crates/studio-web-core/tests; implement safe wasm-bindgen bridge reusing studio-navigation, studio-protocol and studio-ui (FR003–005). Root runs Cargo; package member/lock updates are root-owned.
- [X] T003 [P] Add first-failure Worker admission/request/queued-navigation tests; implement sdk/web/worker.ts with narrow imports, checked buffers, host-authorized root mounts and sequential bounded timers (FR003–005).
- [X] T004 [P] Add first-failure retained DOM/assets tests; implement sdk/web/{renderer,app}.ts, style.css and index.html with closed props, responsive layout, reduced motion and browser history (FR002/005/006).
- [X] T005 Add failing CLI web-target/export/unsupported-kind tests, then implement crates/studio-cli/src/build/web.rs and main.rs flags/build integration; add scripts/build-web-core.ts using matching bindgen and a reusable generated cache (FR001/002).
- [X] T006 Exercise actual example in Chromium: tick/reset/exit, live/back/browser back, direct routes, desktop/mobile assets/layout; fix concrete failures (FR006/007).
- [X] T007 Run focused web/Rust checks and sequential workspace integration gates, document supported target/subset and validation evidence (FR001–007).
- [X] T008 Export sanitized static output plus original .studio source snapshot to separate Sites checkout; push/package exact source, publish privately and verify URL, persist deployment identity (FR008).
- [X] T009 Analyze/converge spec/plan/tasks against implemented behavior; complete remaining gaps and record final evidence (all FRs).

T001 precedes independent T002–004. T005 integrates those foundations. T006–009 close acceptance/publication. The plan explicitly delegates independent slices; root serializes Cargo and owns publishing. Tests precede behavior, and focused results are reused until an integration checkpoint.
