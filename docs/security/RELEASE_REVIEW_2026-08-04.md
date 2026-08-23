# Release Security, Advisory, and License Review

Review date: 2026-08-04

## Security residual-risk review

The milestone threat model remains accurate for the current feature set. The sandbox protects
against hostile guest modules through closed imports, bounded guest memory and execution, checked
ABI copies, atomic UI validation, principal-bound capabilities, opaque-reference scoping, and
terminal recovery. The documented residual risks remain outside the milestone guarantee: a
compromised host, kernel, compositor, administrator, signing authority, compiler, GPU driver, or
side-channel observer.

The review confirms that milestone one still exposes only deterministic payment and printer
simulators. No production network, filesystem, terminal, printer, or generic hardware capability
has been added.

## Advisory scans

### Rust

`cargo audit` scanned the updated locked dependency graph on 2026-08-04. The previously reported
Wasmtime vulnerability is remediated:

- `wasmtime` is now pinned to `47.0.3`, which is outside the affected range for
  `RUSTSEC-2026-0222`.

The full workspace check, Bun checks, RustSec scan, and Bun audit pass after the upgrade. The scan
still reports five unmaintained transitive crates (`instant`, `paste`, `proc-macro-error2`,
`rustybuzz`, and `ttf-parser`); they are not reported as known vulnerabilities and remain tracked
for upstream replacement.

The scan also reported unmaintained crates (`instant`, `paste`, `proc-macro-error2`, `rustybuzz`,
and `ttf-parser`). These are transitive dependencies; they require maintainer/legal disposition,
but are not reported as known vulnerabilities by the scan.

### Bun

`bun audit` completed successfully with no reported vulnerabilities in the locked workspace
dependency set.

## License inventory

The Cargo metadata inventory is predominantly MIT, Apache-2.0, BSD, ISC, Zlib, or similarly
permissive licensing. The following items require explicit legal confirmation before distributing
the linked binary:

- Zed GPUI and linked Zed crates identify GPL-3.0-or-later licensing in the repository notices.
- The GPUI/Zed pin was upgraded to current upstream `main` revision
  `381953d44897c53c4d252ae30620bafaa7d060b7`; license review must cover this final revision.
- `zlog`, `ztracing`, and `ztracing_macro` report GPL-3.0-or-later metadata.
- `gpui_shared_string` and `gpui_util` do not expose a license field in Cargo metadata and need
  source-license verification.
- `libbz2-rs-sys` reports bzip2-1.0.6 and requires its notice to be included if linked.

This inventory is evidence for counsel/release-owner review, not a legal conclusion. The release
checklist must remain open until the final linked binary, notices, and distribution obligations
are approved.
