# GPUI dependency alignment — 2026-10-31

The latest published releases checked on this date are GPUI Kit 0.8.1 and the matching
GPUI (`gpui-pre`) family at 0.3.10. Applications depend on the Kit facade and let it select
the matching GPUI platform crates. The default upstream backend is used; GPUI Fast is not
enabled by this change.

| Project under `/var/home/swhite/Development` | Previous Kit requirement | Current Kit / GPUI lock | Compilation check |
| --- | --- | --- | --- |
| `studio-platform` | vendored 0.7.0-studio.1 | 0.8.1 / 0.3.10 | Runtime and Designer, all targets: passed |
| `PartyTime` | 0.7.0 | 0.8.1 / 0.3.10 | App and API, all targets: passed |
| `Party-Platform` | 0.7.0 | 0.8.1 / 0.3.10 | Desktop app and API, all targets: passed |
| `sent-platform` | 0.7.1 | 0.8.1 / 0.3.10 | POS app, all targets: passed |
| `studio-os-tv` | 0.8.1 | 0.8.1 / 0.3.10 | All targets: passed |

All five manifests now request `=0.8.1` and their lockfiles contain one matching GPUI family.
Alexia's `gpui-app` directory currently contains an agent host without an active GPUI dependency;
no UI dependency was added to it. No other active GPUI Cargo consumers were found in the parent
development directory.

## Studio platform changes

The old five-crate vendor fork and no-X11 scripts were removed. The Runtime and Designer share
the native display preflight in `studio-components`: Linux accepts Wayland display/socket or
X11 DISPLAY; other native targets delegate connection setup to GPUI. The feature check confirms
Kit enables Linux Wayland/X11 and the platform `font-kit` feature needed for macOS text.

Constitution 2.0.0 and ADR-0004 supersede the original Wayland-only restriction. Historical
milestone/release evidence keeps its original meaning and does not certify this upgrade.

## Verification and limits

Checks used the shared parent Cargo target directory sequentially, with development/test debug
symbols and incremental compilation disabled. The two PartyTime checks ran inside the existing
Fedora development Distrobox with libobs provisioned. Other checks ran on the Linux host.

- Studio: `cargo check -p studio-app -p studio-designer --all-targets` passed; the command also
  refreshed the lockfile after adding the shared helper dependency to Designer.
- Neighboring apps: focused `cargo check --locked ... --all-targets` commands passed as listed
  above. These compile test targets; they do not execute the applications' test suites.
- `cargo fmt --all -- --check`, `scripts/check-platform-features.sh`, and the published-release
  check `scripts/check-upstream-sync.sh` passed for Studio.
- Focused Clippy passed with warnings denied:
  `cargo clippy --locked -p studio-components -p studio-app -p studio-designer --all-targets -- -D warnings`.
- Studio's foundation/documentation tests passed (27 tests).
- `cargo test --locked -p studio-components --lib platform::tests` passed (3 tests). The
  Linux host used its existing native-library shim directory through `LIBRARY_PATH` for this
  linked test executable; no release packaging was validated.

This is Linux compilation evidence for the 2026-10-31 alignment. No release build, interactive
smoke test, or full macOS/Windows/browser/mobile build was performed in that pass. The Unix-only
reload IPC found during alignment was subsequently addressed on 2026-11-01; see
[reload platform adapter](RELOAD_PLATFORM_ADAPTER.md) for that change's scope and validation.

References: https://gpui-kit.com/docs/getting-started/ and
https://crates.io/crates/gpui-kit/0.8.1 .
