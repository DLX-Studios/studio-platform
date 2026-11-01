# GPUI Kit dependency policy

As of 2026-10-31 Studio uses published `gpui-kit = "=0.8.1"`. Kit selects the matching
`gpui-pre` family at `=0.3.10`, including native platform support and the macOS `font-kit`
feature. Linux includes both Wayland and X11. See [ADR-0004](../architecture/ADR-0004-native-platforms.md).

The old five-crate `0.7.0-studio.1` vendor fork was removed. Its local deltas were dependency
rewrites, Wayland-only feature selection, and publishing metadata; the published facade now
supplies the matching layers directly. Git history preserves the old source and notices.

- Keep protocol mapping and application behavior in Studio crates.
- Depend on the Kit facade; let Kit select matching GPUI snapshots.
- Update the manifest and lockfile together and check renderer compatibility.
- Run `scripts/check-platform-features.sh` to confirm Linux X11/Wayland and macOS font features.
- Run `scripts/check-upstream-sync.sh` to compare locked packages with published releases.

Upstream: https://github.com/longbridge/gpui-kit
Documentation: https://gpui-kit.com/docs/getting-started/
