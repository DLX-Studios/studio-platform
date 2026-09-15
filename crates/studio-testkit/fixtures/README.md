# WebAssembly fixture metadata

Fixtures are built in memory by `studio_testkit::wasm::WasmFixtureBuilder`; generated binary files
are not checked in. Each test records its hostile property in the Rust builder call rather than in
an ambiguous filename.

Supported declarations cover ordered function imports, exported/private functions, multiple
bounded or intentionally unbounded memories and `funcref` tables, deterministic constants,
infinite loops, immediate traps, and raw imported-function calls with adversarial `i32`
pointer/length values. `build()` always emits both newline-terminated WAT and its corresponding
WASM bytes, or a typed encoding error. The builder never instantiates or executes a fixture.
