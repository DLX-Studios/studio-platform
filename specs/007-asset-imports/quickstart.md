# Quickstart: Asset Validation

## 1. Reference matrix

```bash
cargo test -p studio-cli --test asset_icons
```

Expected: referenced-only collection, no phantoms, missing-icon failure with spans,
override wins, determinism, budget enforcement.

## 2. Real package end to end

```bash
bun install --frozen-lockfile
cargo test -p studio-cli --test asset_icons -- --ignored real_package
```

Expected: collection against the pinned `lucide-static` resolves real SVGs byte-identical
to the package files. (Runs in CI where node_modules is installed.)

## 3. Full gate (deferred)

Per the post-008 verification phase.
