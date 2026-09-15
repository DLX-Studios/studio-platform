# Validation Report: Studio Script Developer Experience

**Validation date:** 2026-09-03
**Implementation result:** T001–T011 complete (T012/T013 full gate deferred to the
post-008 verification phase per directive). The language server composes the
Rust `rsvelte` crates with Studio-owned intelligence behind a thin VS Code
client; no JavaScript toolchain package enters the closure.

## Automated evidence (deferred-execution suites)

| Suite | Coverage |
| --- | --- |
| `format_golden` | Deterministic formatting, idempotence, rejection degrades to message, unknown documents |
| `lint_fixtures` | Subset violations with codes/spans, unknown catalog kinds, clean valid sources, legacy path preserved |
| `projection_maps` | Definition jumps, rename across all sites (invalid names refused), references coverage, unknown positions |
| `isolation` | Malformed/oversized inputs never crash any request; formatter rejection untouched; compiler answers identically with tooling present |
| `intelligence` (unit) | 100-entry catalog, every entry resolves through the protocol |
| Extension `tsc` | `editors/vscode` typechecks clean |

## Design notes

- `.studio` sources map onto the Svelte pipeline inside the Studio adapter; the
  filename never leaks into formatted output.
- Upstream diagnostic codes pass through verbatim; Studio codes stay `STUDIO3xx`.
- The client (`editors/vscode/`) spawns the Rust binary over stdio and degrades
  to highlighting plus local commands without it.
- Vendored `rsvelte_fmt`, `rsvelte_formatter`, `tailwind_class_order`,
  `rsvelte_lint`, `rsvelte_diagnostics`, `rsvelte_preprocess` (manifest-only),
  `rsvelte_projection` at the pinned rev with manifest-only lint deltas.

## Scope and limitations

Packaged-extension manual verification (activation, degraded mode) belongs to
the human verification pass. Full workspace gates run in the post-008 phase.

## Post-008 verification gate (2026-09-04, commit `POST008`)

All green except the pre-accepted Sway environment failure:

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | clean, zero warnings |
| `cargo test --locked --workspace` | **1170 passed, 0 failed** |
| `bun run check` | exit 0 |
| `bun test` | 38 pass, 1 fail — `wayland_startup` only; `sway` binary absent on this KDE machine (pre-accepted) |
| `cargo test -p studio-language-server` (quickstart 1) | all 8 targets green, incl. `stdio_roundtrip` |
| Extension `tsc -p ./` | clean after tsconfig fix (`moduleResolution: node` to match `module: commonjs`) |

Fixes the gate surfaced (all in uncommitted post-008 work, now committed):

- Vendored `rsvelte_fmt`, `rsvelte_formatter`, `rsvelte_lint` `Cargo.toml`
  files declared `[[bench]]` targets whose `benches/` dirs were pruned;
  stanzas removed (manifest-only delta, recorded in `UPSTREAM.md`).
- `studio-language-server`: Svelte-shaped completion/hover/definition now
  resolve workspace symbols inside `{...}` expressions (tokens, `$item`
  schema fields, plugin surfaces) instead of answering catalog-only;
  tag-position completion also offers workspace components. The dead
  speculative `record_generated_mapping` helper (whose `insert_generated`
  wrapper could never create a segment on an empty map) was removed in
  favor of engine-produced maps; the unit test now pins the true
  empty-map no-op contract.
- `editors/vscode/tsconfig.json`: `moduleResolution` `bundler` → `node`
  (pre-existing mismatch; extension typechecks clean).
