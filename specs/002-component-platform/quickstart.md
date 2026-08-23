# Component Platform Validation Quickstart

## Prerequisites

- Locked Rust and Bun dependencies installed.
- A native Wayland session for graphical checks; X11 is not supported.

## Contract and SDK checks

```bash
cargo test --locked -p studio-protocol -p studio-components
cargo test --locked -p studio-protocol --test component_catalog_v1
cargo test --locked -p studio-components --test catalog_mapping
cargo test --locked -p studio-components --test component_catalog
bun run generate:protocol
git diff --exit-code -- protocol sdk/assemblyscript/assembly/generated
bun test sdk/assemblyscript/tests/component-catalog.test.ts
```

## Native and security checks

```bash
./scripts/check-no-x11-features.sh
cargo test --locked --workspace
bun run check
bun test
```

## POS demonstration

```bash
bun run build:pos
cargo run --locked -p studio-app -- --dev examples/pos-desktop/build/pos-desktop.studio
```

Verify the selected component batch in the catalog and order summary, keyboard focus order,
accessible labels/states, reduced motion, invalid-operation diagnostics, and payment overlay
ownership. Hardware-specific performance certification is deferred.
