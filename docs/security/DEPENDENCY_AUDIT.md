# Dependency and Supply-Chain Audit

Audit date: 2026-08-04. Inputs: `Cargo.lock`, `bun.lock`, workspace manifests, the vendored
`gpui-component` tree, `cargo tree --locked -e features,no-dev`, `bun pm ls --all`, `cargo audit`,
and `bun audit`. Detailed findings are recorded in
`docs/security/RELEASE_REVIEW_2026-08-04.md`.

## Security-critical direct dependencies

| Dependency | Pin/control | Purpose | Review result |
| --- | --- | --- | --- |
| Wasmtime | exact `45.0.3`, limited features | sandbox execution | no WASI; Cranelift/runtime/std only; policy and resource tests pass |
| wasmparser | exact `0.255.0` | pre-instantiation policy | proposals/imports/ABI fail closed |
| ed25519-dalek | exact `3.0.0`, zeroize | bundle signatures | raw 64-byte signatures; strict verification |
| serde/serde_json/schemars | lockfile + closed structs | protocol/schema | unknown fields denied; bounded before mutation |
| zip | exact `8.6.0`, stored/deflate features | bundle inspection | stored-only shipping output; paths/metadata/limits audited |
| getrandom/zeroize | exact pins | opaque references and erasure | CSPRNG handles; terminal registry cleanup |
| GPUI/GPUI platform | exact Git revision | native UI | current Zed main pin, default features off; Wayland only; release ELF has no X11/XCB |
| gpui-component | vendored full upstream fork | native controls, themes, accessibility | Wayland-only GPUI pin; full vendor delta recorded |
| AssemblyScript/TypeScript/Bun types | `bun.lock` | guest SDK/tooling | build-time only; starter and POS compile under lockfile |

## Findings and release disposition

- No generic network, filesystem, subprocess, browser/DOM, or WASI capability is linked into the
  guest boundary.
- Duplicate transitive versions are predominantly GPUI graphics/text dependencies and do not
  expand guest authority. They remain a size/update concern.
- The GPUI license compatibility of the final distributed binary is a release blocker requiring
  legal confirmation; it is explicitly called out in `THIRD_PARTY_NOTICES.md` and the checklist.
- Fuzzing, hostile fixtures, redaction tests, deterministic packaging tests, and no-X11 checks are
  mandatory release gates.
- Dependency upgrades require a new feature-tree, license, vulnerability, and behavior audit plus
  regeneration of `Cargo.lock`/`bun.lock`; floating production dependencies are prohibited.
- `cargo audit` is clean of known vulnerabilities after upgrading the exact Wasmtime pin to
  `47.0.3`; five unmaintained transitive crates remain warnings for future upstream replacement.
- `bun audit` reports no vulnerabilities for the locked workspace dependencies.

Residual ecosystem risk includes compiler/runtime vulnerabilities, native compositor/GPU driver
bugs, and build-tool compromise. Lockfiles reduce drift but do not replace advisory monitoring or
reproducible release infrastructure.
