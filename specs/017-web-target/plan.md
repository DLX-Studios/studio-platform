# Plan: web target and hosted countdown

Reuse the existing .studio composer and routed AssemblyScript emitter. Add CLI --target web with a static export at build/web. Browser host source lives in sdk/web; Bun bundles its TypeScript and copies the HTML/CSS. A new studio-web-core Rust cdylib/rlib uses wasm-bindgen0.2.127 to expose the existing ProjectRouter and TimerSchedule. A dedicated Worker loads this trusted host bridge and the compiled guest module; the main document owns DOM, accessibility, CSS transitions and the watchdog.

## Ownership and contracts

- Root: CLI build/export, source manifest, integration/browser validation, Sites registration/source synchronization/publication. No edits to unrelated user work.
- Router slice: crates/studio-web-core Cargo/source/tests. Pure Rust shared core API with Wasm bindings; no filesystem/network/UI access.
- Worker slice: sdk/web/worker.ts and worker-contract tests. Existing protocol messages, host-admitted route mounts, queued navigation responses outside Wasm callbacks, checked pointers, handler event admission, timers polled on host monotonic time, bounded callback loops.
- Renderer slice: sdk/web/app.ts, renderer.ts, style.css, index.html and renderer tests. Closed DOM kinds/props, safe textContent/attributes/assets, keyed native identity, CSS reduced motion, browser history bridge and responsive viewport cover.

The root serializes Cargo. The plan and implementation workflows explicitly delegate these independent bounded slices to separate agents; only root runs Cargo/formatter. New code follows first-failure tests, then focused green checks before integration.

## Shared browser contract

The Rust wasm-bindgen class WebHost takes routing JSON, timer JSON (empty pages allowed), initial concrete route, and monotonic now_ms. Methods current(), navigate(command_json, now_ms), observe(now_ms, completed_json), poll_one(now_ms, completed_json) return JSON strings or throw a String error. current/navigate return {route,id,params}; poll_one returns JSON null or {node_id,event}. Parameters are finite nonnegative integer milliseconds, capped safely; completed_json is an array of timer IDs. State remains in guest Wasm; Rust owns matching/history/scheduling. Timer metadata path assets/studio/timers.json and routing path assets/studio/routes.json remain unchanged.

Main->Worker messages: {type:'init',route}, {type:'input',node_id,event:'pressed'}, {type:'navigate',command}, {type:'poll'}, with numeric request id. Worker->main: {id,type:'commit',root,route,history_operation}, {id,type:'idle'}, or {id,type:'error',message}. Main sends one request at a time and terminates the Worker after a bounded active-request deadline; it does not reset watchdog based on unsolicited messages. Worker batches timer callbacks (<=32) and the main document only updates on committed changes.

Build exports web-config.json with schemaVersion1, the manifest's admitted asset paths, routing and timer declarations (or URLs under same origin), module.wasm, host-core JS/Wasm and bundled worker/app. Only compiled author-controlled static artifacts are supported; native signed bundle admission/fuel certification is not claimed. No external navigation/action capability is exposed. Browser host validates existing guest messages and mounted node tree using closed kinds/props, bounded depth/count/message bytes, safe asset paths and stable identities; failed operations retain the committed document.

## Constitution and validation

Existing protocol remains authoritative. Browser-specific isolation uses a Worker and termination deadline, not a claim of Wasmtime epoch/fuel support. No privileged host capability is added. Original native source/build paths remain intact. Red/green Rust core, worker security/ownership, DOM/asset and CLI export tests precede implementation. Real Chromium verifies the actual full example at desktop/mobile sizes, count/timer/exit/reset/history/deep links and no loading errors. Final Rust format/focused tests/Clippy and web type/browser checks use one Cargo process; workspace integration is sequential when warranted. No Cargo release profile build.

Sites uses a separate sanitized static source checkout, preserving its owner-private default. Push exact prepared output/source state, package through site-workflow helper, save/deploy, then poll a nonterminal deployment. Native success with URL is the publication gate.
