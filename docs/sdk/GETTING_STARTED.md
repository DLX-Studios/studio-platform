# Studio AssemblyScript SDK Quickstart

The checked-in `examples/starter` plugin is the smallest complete Studio plugin: a counter, a
derived exact-money total, one retained mount, and one targeted property patch. The documented
flow is designed to finish well inside ten minutes on a prepared development machine.

## Prerequisites

- Rust 1.93 (the repository toolchain selects it automatically)
- Bun 1.3 or newer
- a native Wayland session when opening the graphical host; Studio intentionally has no X11 or
  XWayland fallback

## Build and package

From the repository root:

```bash
bun install --frozen-lockfile
bun run build:starter
```

This compiles `examples/starter/assembly/index.ts` and writes a deterministic signed example bundle
to `examples/starter/build/starter.studio`. For a local unsigned bundle made with the Rust CLI:

```bash
cargo run -p studio-package --bin studio-pack -- \
  --manifest examples/starter/manifest.json \
  --module examples/starter/build/starter.wasm \
  --output examples/starter/build/starter-dev.studio \
  --dev
```

Unsigned output is rejected unless `--dev` is explicit, and the host keeps a persistent
development warning while it is loaded.

## Launch

In a native Wayland session:

```bash
cargo run -p studio-app -- --dev examples/starter/build/starter.studio
```

Activate **Increment** with Enter or Space. Only the `total.text` property is patched; the native
tree, focus, and unrelated state remain retained.

## Validate the complete quickstart

```bash
./scripts/test-starter-quickstart.sh
```

The script installs only lockfile-pinned packages, compiles and packages the starter twice as
needed by the smoke test, verifies mount/patch behavior, demonstrates the stable invalid-target
diagnostic fixture, and fails if the flow takes ten minutes or more.

## Authoring rules

- Use stable, unique node IDs and closed builders from `@studio/sdk`.
- Keep money as integer minor units plus an explicit currency.
- Register typed events; never put credentials in plugin state or messages.
- Use `SecretInput` and opaque authorization references for sensitive input.
- Declare only `payment.simulate` or `printer.simulate` when the plugin actually uses them.
- Treat every host rejection code as recoverable input to developer tooling; do not retry blindly.
