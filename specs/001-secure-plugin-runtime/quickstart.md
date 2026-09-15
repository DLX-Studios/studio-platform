# Validation Quickstart: Secure Native Plugin Runtime

This guide is the executable acceptance path for milestone one. Commands for future crates become
required as their tasks land; the foundation checks can run immediately. Run from repository root.

## Prerequisites

- x86_64 Linux in a native Wayland session
- Rust toolchain from `rust-toolchain.toml`
- Bun compatible with the checked lockfile
- C/C++ build tools and GPUI's documented Linux system libraries
- `cargo tree`, `ldd`, and a headless Wayland compositor such as Weston or Sway for platform CI
- The `STUDIO-BENCH-1` device profile from `plan.md` for performance acceptance

Do not set `DISPLAY` to make a test pass. X11 and XWayland are unsupported.

## 1. Install and Verify the Foundation

```bash
bun install --frozen-lockfile
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
bun run check
bun test
```

Expected: formatting, lint, Rust tests, AssemblyScript checks, TypeScript checks, and foundation
tests pass with locked dependencies.

## 2. Prove the Wayland-Only Gate

```bash
./scripts/check-no-x11-features.sh
env -u DISPLAY cargo build --locked --release -p studio-app
./scripts/check-no-x11.sh target/release/studio-app
STUDIO_APP_BINARY="$PWD/target/release/studio-app" ./scripts/test-headless-wayland.sh
```

Expected:

- The Cargo feature graph and linked binary contain no X11/XCB client path.
- Starting without a native Wayland endpoint exits with code 2 and a controlled message.
- No X11 or XWayland fallback starts.

Under native or headless Wayland, launch `target/release/studio-app`. The completed foundation
gallery must demonstrate text, button, non-secret text input, scroll, popup, focus traversal,
accessible labels/states, one host-scheduled animation, and reduced-motion behavior.

## 3. Validate Generated Contracts

After the protocol generation slice lands:

```bash
cargo test -p studio-protocol
bun run generate:protocol
git diff --exit-code -- protocol sdk/assemblyscript/assembly/generated
bun test tests/contracts
```

Expected: Rust types, JSON Schemas, AssemblyScript bindings, and golden fixtures agree. Unknown
fields, unsupported versions/kinds/properties, invalid trees, duplicate IDs, oversized messages,
out-of-order patches, and partially invalid batches are rejected without mutation.

## 4. Validate the Sandbox

After `studio-wasm` and hostile fixtures land:

```bash
cargo test -p studio-wasm
cargo test -p studio-testkit
cargo test -p studio-app --test plugin_recovery
```

Expected fixtures prove:

- WASI and unknown imports, invalid exports, unsupported proposals, extra memory, and excess table
  declarations fail before guest execution.
- Invalid pointers/lengths, arithmetic overflow, invalid UTF-8, oversized messages, traps, infinite
  loops, fuel exhaustion, epoch interruption, and memory growth terminate only the guest.
- The Studio shell remains responsive and displays a host-owned error surface.
- Manual restart creates a fresh instance ID and restores no guest/UI/navigation/action/secret
  state.

## 5. Validate Retained UI and SDK Reactivity

```bash
cargo test -p studio-ui -p studio-components
bun test sdk/assemblyscript/tests
cargo test -p studio-app --test catalog_cart
```

Expected:

- A valid mount commits once; invalid mounts render nothing.
- Patch batches commit all operations or none.
- A property patch changes only its dependency path and preserves unrelated focus, scroll, text
  entry, and node identity.
- SDK state updates invalidate only dependents, batches coalesce writes, derived values memoize,
  removed bindings dispose effects, and cycles terminate with a stable error.
- `SecretInput` never sends entered bytes to the guest.

## 6. Validate Navigation and Animation

```bash
cargo test -p studio-navigation
cargo test -p studio-app --test checkout_navigation
```

Expected: nested and parameterized routes, lazy screens, push/replace/pop/pop-to/reset, not-found,
depth limit, guards, and timeout behavior pass. Push/pop/replace transitions complete with host
timing; reduced motion produces the same final state with zero movement duration.

## 7. Build and Verify Bundles

```bash
bun run build:pos
cargo test -p studio-package
cargo run -p studio-package --bin studio-pack -- \
  --manifest examples/pos-checkout/manifest.json \
  --module examples/pos-checkout/build/pos-checkout.wasm \
  --output examples/pos-checkout/build/pos-checkout-dev.studio \
  --dev
```

Expected: repeated packaging produces identical canonical content/signing input. A valid trusted
bundle passes. Mutated manifest/module/asset/signature, unknown fields, unsafe paths, duplicates,
symlinks, decompression bombs, excess sizes, versions, capabilities, imports, or exports fail
before compilation.

Developer mode validation:

```bash
cargo test -p studio-app --test plugin_launch unsigned_development_launch
```

Expected: only the explicitly selected local bundle runs, a persistent host-owned untrusted
indicator remains visible, and every non-signature sandbox/capability/resource control stays on.

Production selection validation uses an administrator-provisioned absolute path:

```bash
cargo test -p studio-app --test plugin_launch valid_signed_bundle
```

Expected: path location grants no trust; the complete production signature, identity,
compatibility, capability, module, and resource validation still runs before execution.

## 8. Validate Secrets and Simulated Actions

```bash
cargo test -p studio-security -p studio-actions
cargo test -p studio-app --test redaction --test protected_payment
cargo test -p studio-actions --test payment_simulator --test printer_simulator
```

Expected:

- Raw PIN fixtures never appear in guest-visible bytes, guest memory captures, logs, diagnostics,
  snapshots, receipts, or persisted files.
- Wrong-principal, wrong-instance, wrong-purpose, wrong-session, expired, reused, and random handles
  fail identically.
- Trusted confirmation binds merchant, amount, and currency.
- Amount suffixes `00`, `01`, `02`, and `03` produce approved, declined, timeout, and unavailable.
- Repeated idempotency keys create one simulated transaction and return the original result.
- Printer simulation accepts only approved structured receipts and creates one preview job.

## 9. Run the POS Acceptance Flow

Run the automated signed reference flows, then repeat the documented manual accessibility checks
under native Wayland:

```bash
bun run build:pos
cargo test -p studio-app --test catalog_cart --test checkout_navigation --test pos_checkout
cargo test -p studio-app --test plugin_recovery --test compositor_disconnect --test accessibility
```

1. Open catalog, search/filter, add items, change quantities, and confirm exact integer totals.
2. Navigate catalog → cart → checkout; use back/forward behavior and verify route-local state.
3. Enter a PIN in the host-owned field, inspect trusted confirmation, and run all four simulator
   outcomes.
4. Retry recoverable failures safely; verify a pending payment cannot be abandoned silently.
5. On approval, verify receipt values and open the host-owned printer preview.
6. Trigger a guest trap, manually restart, and verify a completely fresh plugin instance.
7. Disconnect the compositor in the headless harness and verify cancellation, secret revocation,
   plugin termination, clean exit, and no automatic restoration.

The complete catalog-to-receipt flow must take under two minutes using only visible labels and
must be operable entirely by keyboard.

## 10. Performance and Hardening

```bash
cargo test --locked --workspace --release
bun test
./scripts/fuzz-smoke.sh
./scripts/benchmark-acceptance.sh
```

The formal `STUDIO-BENCH-1` run is deferred for the current milestone. The acceptance benchmark
may be run on a development host for informative results; record exact CPU, memory, GPU, kernel,
compositor, resolution, and build profile. Warm launch uses 5 warm-ups and 30 measured launches. Each fixed
catalog/cart/navigation operation uses 10 warm-ups and 100 samples, measured from host event
receipt to presentation of the resulting frame with monotonic timestamps. Acceptance requires
warm first frame under 150 ms, interaction p95 under 100 ms, property patch p95 under 2 ms,
100-property batch p95 under 8 ms, normal transitions at 60 FPS, and no repeated guest/render
messages while idle.

## Release Evidence

Before milestone acceptance, archive:

- locked dependency and Cargo feature graphs;
- release linkage report and headless Wayland result;
- contract-generation clean diff;
- unit, contract, integration, security, fuzz-smoke, and performance results;
- secret-isolation scan results;
- updated threat model, upstream extraction ledger, notices, and any component fork deltas;
- traceability from every FR/SC to an automated test or explicit manual native check.
