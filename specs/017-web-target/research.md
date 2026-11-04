# Decisions

- Rust wasm32-unknown-unknown target and wasm-bindgen CLI are available. Workspace bindgen dependency is0.2.127; use the matching official prebuilt CLI rather than upgrade unrelated workspace versions. Rust target reference: https://doc.rust-lang.org/stable/rustc/platform-support/wasm32-unknown-unknown.html
- Reuse Rust ProjectRouter and TimerSchedule through a bridge; do not port the matcher to another browser implementation.
- Browser lacks Wasmtime's fuel/epoch APIs; a Worker plus parent deadline keeps blocking guest code away from the DOM. https://developer.mozilla.org/en-US/docs/Web/API/Worker/terminate
- Site registration is appgprj_6aea46cdc8108191aefd8ef7d101500d, persisted immediately in the separate Sites checkout manifest. Default private audience is retained. No secrets enter source or logs.
