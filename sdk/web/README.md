# Studio static web host

`studio build <project> --target web` composes the same `.studio` sources as
native Studio and writes a static application to `<project>/build/web`.
It emits the app guest Wasm, the Rust navigation/UI/timer host Wasm bridge,
worker, retained DOM renderer, CSS, metadata and admitted assets.

Install the workspace's locked Bun dependencies and the `wasm32-unknown-unknown`
Rust target. The bridge uses `wasm-bindgen-cli 0.2.127`, matching Cargo.lock.
Set `STUDIO_WASM_BINDGEN` when that executable is outside PATH. The CLI also
uses a matching executable cached under `.scratch/tools/wasm-bindgen` when
present. `scripts/build-web-core.ts` fingerprints Rust sources and Cargo.lock,
so subsequent web exports reuse the generated bridge in `.scratch/web-core`.
Use `STUDIO_WEB_CORE_DIR` only to select an already built bridge directory.
Cargo builds use the configured shared target; serialize web bridge builds
with other Cargo commands.

```sh
studio build examples/stream-countdown --target web
bun scripts/serve-web.ts examples/stream-countdown/build/web 5180
```

Open `http://127.0.0.1:5180/`. Static hosting needs an index fallback for
parameter/catch-all deep links; fixed routes also receive their own index.html.
The export contains `_redirects` for compatible static hosts.

The current adapter supports Box, Scaffold, Column, Row, Text, Image, Button
and Tag with the exact property names in `contract.json`. The CLI rejects
unsupported kinds/properties before replacing a previous export. Source
components/layouts, route parameters, host navigation and source-only Timer
behavior use the same compiler and Rust router/scheduler as native Studio.
Native capabilities and arbitrary guest bundle uploads are unsupported.

Guest code runs in a dedicated Worker, with bounded messages/queues/memory and
checked imports. A parent watchdog terminates an active request after two
seconds (ten seconds for startup). These are browser controls; this target
does not claim Wasmtime fuel/epoch metering or native signed-bundle admission.
Rust protocol/UI validation remains authoritative before a retained tree can
reach the DOM. Assets require exact manifest admission; markup uses textContent
and a closed renderer. The renderer retains element identity for updates,
including the clock's pulse/fade and focused buttons. Browser reduced-motion
preferences remove visual transitions while preserving completion semantics.

Checks:

```sh
bun test ./sdk/web/tests/renderer.test.ts ./sdk/web/tests/app.test.ts ./sdk/web/tests/worker-contract.test.ts
bun run node_modules/.bin/tsc --noEmit -p sdk/web/tsconfig.json
# After installing Playwright Chromium, with the local server running:
STUDIO_WEB_BROWSER=1 bun test ./sdk/web/tests/browser.acceptance.test.ts
```

`STUDIO_BROWSER_PATH` selects an existing Chromium executable;
`STUDIO_WEB_URL` and `STUDIO_WEB_SCREENSHOT_DIR` override the QA origin/output.
The browser acceptance script exercises the actual compiled countdown at desktop
and mobile sizes, including tick/reset/canceled exit, interpolated fade,
delayed navigation, state retention, browser history, deep links and reduced motion.
