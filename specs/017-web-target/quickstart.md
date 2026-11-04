# Build and acceptance

With matching wasm-bindgen and wasm32-unknown-unknown installed, run studio build examples/stream-countdown --target web. Serve build/web with an SPA fallback for direct routes. Verify countdown, reset, delayed live transition, back/history and a direct /live request. Sites export is static with index fallback. See validation-report.md for actual commands and hosted URL.
