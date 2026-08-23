# Studio Platform

This workspace follows the modular Studio Platform layout. The current Rust implementation remains
available at the compatibility crate paths under `crates/`; the target ownership boundaries are
represented by `apps/`, `crates/studio-core/`, `crates/studio-host/`, `crates/studio-components/`,
`protocol/`, and the gallery example directories.

The complete migration map is [docs/architecture/MIGRATION_INVENTORY.md](docs/architecture/MIGRATION_INVENTORY.md).

The root of this repository is the canonical public platform workspace. The private website is
intentionally absent and remains a separate repository.

---

# Studio Runtime

**A sandboxed, native business-app runtime — build desktop, mobile, and web products with Rust + AssemblyScript, or let AI agents build them for you.**

Studio ships one host for many plugins. A POS checkout is just the first example. The same protocol powers a streaming studio that multicasts and merges chats, a social post manager that publishes and aggregates comments across platforms, an e-com storefront with Stripe, and any workflow where isolated plugins compose through typed host capabilities.

> **Plugins are the product.** `x-auth`, `stripe`, `printer-simulate`, `payment-simulate` — small, signed, capability-scoped modules that any other plugin can depend on without sharing secrets or filesystem access.

---

## Why Studio

- **Host-owned everything.** Navigation, secrets, filesystem, networking, and rendering live in Rust. Guest Wasm (Wasmtime, no WASI) only emits a declarative UI tree and targeted patches.
- **Retained native UI.** Flutter-like primitives (`Card`, `Text`, `Button`, `Select`, `SecretInput`, 40+ more) map to real GPUI widgets. No canvas, no HTML — `cargo fmt` and `cargo clippy -D warnings` stay green.
- **Versioned, closed contracts.** `studio-protocol` is the single source of truth; JSON Schema and AssemblyScript bindings are generated from it. Unknown fields, capabilities, or imports are rejected.
- **Wayland-only, deterministic.** No X11/XCB linkage (verified by `check-no-x11.sh`), 16 MiB guest memory, 15M initialization fuel, 10M event fuel / 50 ms epoch per call, atomic patch batches, and an in-memory host for checkout/receipt/print simulation.
- **Test-first.** Every slice starts red: `cargo test --workspace` + `bun test` must stay green before a task is marked done.

---

## Current status

**Milestone 001 — Secure Plugin Runtime — converged and committed as `347643a`.**

Host, sandbox, protocol, retained renderer, SDK, navigation (`CheckoutRouter`/`NativeCheckoutShell` wired into the live GPUI window with route bar + trusted confirmation overlay), packaging, and the `pos-desktop` reference flow (catalog → cart → checkout → receipt → print preview) are integrated. Gates that pass:

```bash
cargo fmt --all -- --check          # 0 diff
cargo clippy --workspace -- -D warnings # 0 warnings
cargo test --locked --workspace     # ~120 tests, incl. native_shell_checkout / accessibility / manifest
bun run check && bun test           # 55 pass, 285 expects
./scripts/check-no-x11-features.sh && ./scripts/check-no-x11.sh
```

Formal `STUDIO-BENCH-1` (N100, 8 GiB, 1920×1080 Weston) and human a11y/legal sign-off in [validation-report.md](specs/001-secure-plugin-runtime/validation-report.md) remain before publication. Live phase status: [docs/ROADMAP.md](docs/ROADMAP.md).

---

## Stats at a glance

| Area | Value |
|------|-------|
| Rust workspace | 12 crates (`studio-app`, `studio-cli`, `studio-protocol`, `studio-components`, `studio-actions`, `studio-wasm`, `studio-security`, `studio-package`, `studio-ui`, `studio-navigation`, `studio-testkit`, `studio-script`) + vendor `gpui` |
| Rust LOC | ~30k (12,464 lib + 17,211 app/cli) |
| JS/SDK tests | 55 pass, 11 files |
| Reference bundle | `pos-desktop.wasm` 33 KB, `pos-desktop.studio` 876 KB (signed zip, deterministic) |
| Guest limits | 16 MiB, 15M initialization fuel, 10M event fuel, 5k nodes, depth 64, 512 ops/batch, 16 pending actions |
| Performance goals | Warm launch <150 ms, interaction p95 <100 ms, property patch p95 <2 ms, 60 FPS transitions |

`target/release/studio-app` builds Wayland-only (`-D warnings` clean) and runs headless under `sway`/`weston` for CI.

---

## The ecosystem — more than POS

POS proves the trust model. The same host runs:

- **Stream Studio** — one plugin captures, one multicasts to YouTube/Twitch/TikTok, one merges chats, one overlays alerts. Each capability (`camera`, `rtmp`, `chat-aggregate`) is a separate signed plugin.
- **Post Manager** — compose `x-auth` + `tiktok-auth` + `scheduler` to draft once, publish everywhere, and aggregate comments into one inbox.
- **E-com** — `catalog` + `cart` + `stripe` (host-owned `payment-simulate` today, real Stripe capability tomorrow) + `printer-simulate` for receipts.
- **Your idea** — any business workflow where UI, secrets, and hardware must stay isolated but composable.

Developers can write plugins by hand, generate them with an LLM, or mix both. Studio Script (below) makes the authoring feel like Svelte; production still ships as signed Wasm.

---

## Quick start

```bash
bun install --frozen-lockfile
cargo build --locked --release -p studio-app   # Wayland-only binary
bun run ./scripts/build-example.ts pos-desktop  # asc → wasm → .studio (deterministic)
./target/release/studio-app --dev examples/pos-desktop/build/pos-desktop.studio

# verify
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
bun run check
bun test
bun run test:all   # = cargo fmt + clippy + cargo test + bun check + bun test + no-X11 + headless wayland
```

Headless CI: `STUDIO_APP_BINARY=./target/release/studio-app ./scripts/test-headless-wayland.sh`

---

## Architecture

```
Plugin (AssemblyScript → wasm) ──emit──► Studio Protocol (JSON) ──validate──► Rust Host
   mount / patch / action                                                    ├─ wasmtime (no WASI)
   opaque secret refs                                                        ├─ GPUI retained widgets
                                                                             ├─ CheckoutRouter / NativeCheckoutShell
                                                                             └─ TrustStore + simulators
```

- `studio-protocol` — closed schemas, `PROTOCOL_VERSION = 1`
- `studio-ui` / `studio-components` — retained registry, focus, scroll, a11y labels
- `studio-wasm` / `studio-security` — sandbox, fuel, opaque `authorization_ref`
- `studio-package` — deterministic zip, Ed25519, `ManifestPolicy { 16 MiB, 10M event fuel }`
- `studio-app` — `FoundationGallery` + `NativeCheckoutShell` (host-owned `Route: /cart` bar, `Trusted Studio confirmation` dialog on `/checkout/payment`)

The compatibility-preserving modularization map, dependency graph, generated-file ownership, and
public/private repository boundary are recorded in
[MIGRATION_INVENTORY.md](docs/architecture/MIGRATION_INVENTORY.md).

---

## Studio Script — Svelte-like authoring, native result

**Coming in `004` — Rust development IR, production Wasm.** Author `.studio` files like Svelte, but tags are Studio components:

```svelte
<script lang="ts">
  let { name, price, available = true } = $props();
  let quantity = $state(1);
  function addToOrder() { emit("add-to-order", { productId: name, quantity }); }
</script>

<Card id="product-card" padding={12}>
  <Text id="product-name" typographyRole="label">{name}</Text>
  <Text id="product-price">{formatMoney(price)}</Text>
  <Button id="add-button" disabled={!available} onclick={addToOrder}>
    {available ? "Add to cart" : "Unavailable"}
  </Button>
</Card>
```

Dev: `studio dev` watches source inputs, `studio-script` adapts a pinned `rsvelte` AST into typed
Studio IR, and the Rust development runtime atomically swaps compatible IR while preserving retained
GPUI identity. Production lowers the same validated IR to AssemblyScript and compiles it with ASC.
QuickJS is not part of the architecture. Full pipeline:
[docs/STUDIO_SCRIPT_TRANSFORM_PIPELINE.md](docs/STUDIO_SCRIPT_TRANSFORM_PIPELINE.md).

File convention (planned): `app.studio.ts`, `components/*.studio`, `routes/*.studio`, generated `assembly/routes.generated.ts` (`declaredRoutes`).

---

## Roadmap

Ordered in [docs/ROADMAP.md](docs/ROADMAP.md) — one Spec Kit feature at a time:

1. **001 Secure Plugin Runtime** — done (checkpoint + T104 convergence)
2. **002 Unified Native Component Platform** — protocol/catalog, `Scaffold/AppBar/Sidebar/NavigationBar/Rail/Drawer`, `Tabs/Breadcrumb/Stepper/Pagination`, keyboard/a11y/reduced-motion demo
3. **003 Studio Toolchain and Development Workflow** — `studio` CLI owns build, `studio-app` owns runtime, debounced watcher (fix `assembly/routes.generated.ts` self-trigger), deterministic outputs
4. **004 Studio Script and Embedded Development Host** — `rsvelte` frontend, typed Studio IR, Rust IR hot swap, ASC lowering; `studio dev` reuses shared host/rendering libraries
5. **005 Runtime Reload and Preview Protocol** — atomic Wasm reload without window restart
6. **006 File-Based Routing** — `routes/` static/nested/param/not-found, deterministic registry
7. **007 Asset Imports and Lucide Icons** — tree-shaken `lucide-static`, user overrides, size tests

Each feature follows `specify → clarify → plan → tasks → implement (red-green) → analyze → converge`.

---

## Examples

- `examples/pos-desktop` — 12-dish `pos-desktop` with `lucide` + `routes/` experiment (reference flow: catalog → cart → checkout → receipt → print preview).
- `examples/starter` — minimal mount/patch loop.
- `examples/starter` — minimal mount/patch loop.

---

## Specification workflow

`v0.15.2` Codex skills under `.agents/skills`, constitution under `.specify/memory/constitution.md`:

```text
$speckit-constitution
$speckit-specify
$speckit-clarify       # required for security-sensitive features
$speckit-plan
$speckit-tasks
$speckit-analyze       # required before security-sensitive implementation
# implement each task test-first
$speckit-converge
```

`$speckit-implement` respects `[P]` parallelism and `Phase` dependencies in `tasks.md`.

---

## Contributing

Studio is built for contributors — human or agent:

- Small, traceable slices (≤5 files except vendored `gpui-component` deltas)
- `cargo fmt`, `clippy -D warnings`, `cargo test`, `bun test` must pass before PR
- No `unsafe`, no X11, no WASI; every guest message is validated
- See [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) and [.specify/memory/constitution.md](.specify/memory/constitution.md)

---

## License

Apache-2.0 — see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for `gpui`/`gpui-component` pins.
