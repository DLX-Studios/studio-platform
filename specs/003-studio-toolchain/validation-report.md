# Validation Report: Studio Toolchain and Development Workflow

**Validation date:** 2026-09-03
**Implementation result:** T001–T020 complete. The `studio` binary owns a phased
(validate → compile → package) build pipeline with stable diagnostic codes and documented exit
codes, atomic byte-identical bundle output, a debounced single-flight watcher that excludes
generator outputs, and toolchain-owned packaging with the documented TypeScript entries
delegating to it.

## Automated evidence

| Gate | Result | Evidence |
| --- | --- | --- |
| Formatting | PASS | `cargo fmt --all -- --check` |
| Rust lint | PASS | `cargo clippy --locked --workspace --all-targets -- -D warnings` |
| Rust workspace | PASS | `cargo test --locked --workspace` (exit 0) |
| Build pipeline (US1) | PASS | `cargo test -p studio-cli --test build_pipeline` — clean build exit 0; repeated builds byte-identical; compile failure exits 11 with `BUILD_COMPILE_ASC` and leaves the previous bundle untouched; missing-asset failure exits 12; validate failures exit 10; unknown example exits 2 |
| Watch loop (US2) | PASS | `cargo test -p studio-cli --test watch_session` — generator churn (`routes.generated.ts`, icon writes) never triggers; five rapid saves coalesce into one pending build; single trailing rebuild after in-build changes; cancellation exits 130 with no partial output; missing runtime exits 14 with `BUILD_WATCH_RUNTIME` |
| Packaging shim (US3) | PASS | `cargo test -p studio-cli --test shim_parity` — `build-example.ts` and `studio build` produce byte-identical bundles with identical exit codes; `./scripts/test-starter-quickstart.sh` completed in 147 s (under the ten-minute ceiling) exercising the shim → native chain end to end |
| Diagnostics (US4) | PASS | exit-code classification tests for usage/validate/compile/package/io families; machine-readable JSON diagnostics on stderr with `phase`/`code`/`severity`/`message` |
| Generator stability | PASS | content-stable `generate_routes`/`collect_lucide` proven by the no-rewrite mtime test and repeated-build byte identity |
| JavaScript gates | PASS | `bun run check` exit 0; root `bun test` 38 pass with only the environment-gated `tests/platform/wayland_startup.test.ts` (requires the headless Sway harness; the host runs KDE Wayland, and compositor launch evidence exists in `specs/001-secure-plugin-runtime/validation-report.md`) |
| Format equivalence | PASS | the Rust packager reproduces the previous TypeScript packager's container byte-for-byte at the packaging layer: unpacked comparison of a fresh pos-desktop build against the tracked golden bundle shows identical manifest, all 12 assets, and ZIP layout; `module.wasm` (and its signature) differ only through asc/toolchain drift between build environments, which is outside the packaging boundary |

## Scope and limitations

Exit codes and diagnostic codes are documented in `specs/003-studio-toolchain/contracts/cli.md`
and are additive-only from here. Dev-mode presentation still launches the runtime host binary in
development mode through the launch seam (`research.md` R5); the in-process composition described
in the roadmap belongs to features 004/005. The watcher observes local filesystems only. No new
capabilities, network paths, or storage paths were introduced.
