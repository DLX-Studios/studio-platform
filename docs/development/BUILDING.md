# Building Studio Runtime

## Supported Development Host

Studio milestone one builds and runs on 64-bit Linux with native Wayland. X11 and XWayland are
not supported build or runtime fallbacks.

The repository pins Rust `nightly-2026-03-04`, Bun `1.3.9` in CI, Cargo dependencies through
`Cargo.lock`, and JavaScript dependencies through `bun.lock`.

## Debian/Ubuntu Build Packages

Install the native headers used by the pinned GPUI Wayland build:

```bash
sudo apt-get update
sudo apt-get install --yes \
  libfontconfig1-dev \
  libfreetype-dev \
  libvulkan-dev \
  libwayland-dev \
  libxkbcommon-dev \
  sway
```

Also install `git`, `curl`, `build-essential`, `pkg-config`, and `ldd`/`binutils` if the base
system does not already provide them. Headless platform tests additionally require Weston or Sway.

## Locked Validation

```bash
bun install --frozen-lockfile
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo test --locked -p studio-protocol --test component_catalog_v1
cargo test --locked -p studio-components --test catalog_mapping
cargo test --locked -p studio-components --test component_catalog
bun run check
bun test
bun test sdk/assemblyscript/tests/component-catalog.test.ts
bun run generate:protocol
git diff --exit-code -- protocol sdk/assemblyscript/assembly/generated
./scripts/check-no-x11-features.sh
cargo build --locked --release -p studio-app
./scripts/check-no-x11.sh target/release/studio-app
STUDIO_APP_BINARY=target/release/studio-app ./scripts/test-headless-wayland.sh
```

Build and linkage checks must run with `DISPLAY` unset or empty. Running the application itself
requires a native `WAYLAND_DISPLAY` or `WAYLAND_SOCKET` endpoint.
