#!/usr/bin/env bash
set -e

export DEBIAN_FRONTEND=noninteractive
apt-get update

apt-get install -y \
  build-essential \
  clang \
  lld \
  cmake \
  pkg-config \
  libssl-dev \
  git \
  curl \
  ca-certificates \
  libsqlite3-dev \
  libfreetype6-dev \
  libfontconfig1-dev \
  jq

# Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

rustup component add rustfmt clippy
rustup target add wasm32-unknown-unknown

# sccache
cargo install sccache

# Configure cargo to use sccache + sparse registry for speed
mkdir -p "$HOME/.cargo"
cat << 'EOF' > "$HOME/.cargo/config.toml"
[build]
rustc-wrapper = "sccache"

[registries.crates-io]
protocol = "sparse"
EOF

# Bun (replaces Node/npm entirely — safe for the Svelte frontend too)
curl -fsSL https://bun.sh/install | bash
export BUN_INSTALL="$HOME/.bun"
export PATH="$BUN_INSTALL/bin:$PATH"
bun --version

# Vite+ unified toolchain (vp CLI for dev/build/test)
curl -fsSL https://viteplus.dev/install.sh | bash || true
source "$HOME/.bashrc" || true
vp --version || true

# Node.js 22 (required by the on-machine pi agent)
curl -fsSL https://deb.nodesource.com/setup_22.x | bash -
apt-get install -y nodejs
node --version

# pi coding agent (controlled over RPC by the dashboard daemon)
curl -fsSL https://pi.dev/install.sh | sh
export PATH="$HOME/.local/bin:$PATH"
pi --version || true

# Global system prompt: dashboard-oriented agent behaviour
mkdir -p "$HOME/.pi/agent"
cat << 'EOF' > "$HOME/.pi/agent/SYSTEM.md"
# Studio Builder — on-machine agent

You are the build-machine agent for Studio Builder. You operate a dedicated,
ephemeral Linux builder droplet. The dashboard user chats with you from another
device; your tool calls and messages are rendered inside the dashboard UI.

## Environment
- Repo checkout lives at /root/workspace/studio (already cloned/pulled at boot).
- Rust toolchain + sccache (S3-backed, configured) are ready. Builds are cheap:
  the cache survives this machine being destroyed.
- This machine may be destroyed at any time. Never treat local state as
  precious; commit/push meaningful work before finishing a task.

## Reporting back (dashboard UI contract)
When a step produces something the dashboard should render, emit a fenced
block on its own line:

- \u0060\u0060\u0060ui:status — {"state": "ready" | "agent" | "busy", "note": "..."}
- \u0060\u0060\u0060ui:done — {"summary": "...", "artifacts": ["path", ...]}
- \u0060\u0060\u0060ui:error — {"summary": "...", "hint": "..."}

Keep normal prose short and skimmable; the UI highlights these blocks.
EOF

# Cleanup
apt-get autoremove -y
apt-get clean

echo "=========================================="
echo "Golden image setup complete!"
echo "Next: poweroff the droplet, then snapshot"
echo "=========================================="
