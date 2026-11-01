# Building Studio Runtime

## Supported Development Host

Studio uses published GPUI Kit 0.8.1 with its matching GPUI 0.3.10 platform family.
Linux builds include Wayland and X11; macOS and Windows use upstream native backends.
Dependency alignment enables those backends; release validation on each OS is still required.

The repository pins Rust `nightly-2026-03-04`, Bun `1.3.9` in CI, Cargo dependencies through
`Cargo.lock`, and JavaScript dependencies through `bun.lock`.
The parent `.cargo/config.toml` shares `/var/home/swhite/Development/.cargo-target`;
serialize Cargo commands that use it. For verification, set `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, and `CARGO_INCREMENTAL=0` before the first build.

If the checkout is on vfat or exfat, keep Cargo's target directory on a Linux filesystem. Those
filesystems do not preserve the executable bit required for generated Rust build scripts and can
produce `Permission denied` errors:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
# Development checkouts share ../.cargo-target through ../.cargo/config.toml.
# Override only when a different Unix filesystem is required.
export CARGO_TARGET_DIR="$HOME/.cache/studio-cargo/verify"
```

## Debian/Ubuntu Build Packages

Install the native headers used by the GPUI native platform build:

```bash
sudo apt-get update
sudo apt-get install --yes \
  libclang-dev \
  clang \
  libfontconfig1-dev \
  libfreetype-dev \
  libvulkan-dev \
  libwayland-dev \
  libxkbcommon-dev \
  libxkbcommon-x11-dev \
  libx11-xcb-dev \
  sway
```

`libclang-dev` and `clang` are required by the `surrealdb-librocksdb-sys` bindgen step used by
`studio-host` and `studio-designer`; without them its build script fails with
`Unable to find libclang`.

Also install `git`, `curl`, `build-essential`, `pkg-config`, and `ldd`/`binutils` if the base
system does not already provide them. Headless platform tests additionally require Weston or Sway.

## Run the Designer

The standalone native Designer is separate from the `.studio` runtime host:

```bash
cargo run --locked -p studio-designer
```

If it was built already, launch `"$CARGO_TARGET_DIR/debug/studio-designer"` directly. This
requires a graphical session (Wayland or X11 on Linux).

## Locked Validation

Routine CI reuses the debug profile for its native startup probe, with debug symbols and
incremental compilation disabled. It reports filesystem and artifact usage so exhaustion
can be diagnosed without deleting unrelated caches. Set `STUDIO_APP_BINARY` to the actual
configured target directory when using the shared development cache.

Release builds belong to an explicit release checkpoint (`cargo build --locked --release -p studio-app`);
the debug startup probe does not supply release size or packaging evidence.

```bash
bun install --frozen-lockfile
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked -p studio-protocol --test component_catalog_v1
cargo test --locked -p studio-components --test catalog_mapping
cargo test --locked -p studio-components --test component_catalog
bun run check
bun test
bun test sdk/assemblyscript/tests/component-catalog.test.ts
bun run generate:protocol
git diff --exit-code -- protocol sdk/assemblyscript/assembly/generated
./scripts/check-platform-features.sh
cargo build --locked -p studio-app
STUDIO_APP_BINARY=target/debug/studio-app ./scripts/test-headless-wayland.sh
```

On VFAT/exFAT or another symlink-hostile checkout, replace `bun install --frozen-lockfile` with
`bun install --no-save --backend=copyfile --linker=hoisted`; this keeps dependencies local without
rewriting `bun.lock`.

On Linux, launch requires a nonempty `WAYLAND_DISPLAY`, inherited `WAYLAND_SOCKET`, or
`DISPLAY`. GPUI selects and opens the actual backend. Other platforms do not require Linux
display variables. Headless Wayland remains a Linux smoke test, not an exclusive platform policy.
