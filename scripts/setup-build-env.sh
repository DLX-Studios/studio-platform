#!/usr/bin/env bash
# Provision a Debian/Ubuntu machine to build and test studio-platform.
# Run as root: ./scripts/setup-build-env.sh
set -euo pipefail

if [[ "$(id -u)" -ne 0 ]]; then
  echo "Run as root (sudo ./scripts/setup-build-env.sh)" >&2
  exit 1
fi

export DEBIAN_FRONTEND=noninteractive

apt-get update
apt-get install --yes \
  ca-certificates \
  curl \
  git \
  build-essential \
  pkg-config \
  clang \
  libclang-dev \
  libfontconfig1-dev \
  libfreetype-dev \
  libvulkan-dev \
  libwayland-dev \
  libxkbcommon-dev \
  dbus \
  sway \
  wl-clipboard

# Rust toolchain: version is pinned by rust-toolchain.toml in the repo.
if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none
fi
# shellcheck disable=SC1091
source "$HOME/.cargo/env" 2>/dev/null || source "/root/.cargo/env"

# Bun (repository pins 1.3.9 in CI; any recent 1.x works for tests).
if ! command -v bun >/dev/null 2>&1; then
  curl -fsSL https://bun.sh/install | bash
fi
export PATH="$HOME/.bun/bin:$PATH"

echo
echo "Installed. To build and verify:"
echo "  export PATH=\"\$HOME/.cargo/bin:\$HOME/.bun/bin:\$PATH\""
echo "  bun install --frozen-lockfile"
echo "  cargo build --locked -p studio-designer   # debug Designer"
echo "  ./scripts/test-headless-wayland.sh        # headless Wayland gate (uses sway)"
