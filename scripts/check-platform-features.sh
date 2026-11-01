#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

# Inspect declared Linux backends even when this script runs on a different host OS.
features="$(cargo tree --locked --target x86_64-unknown-linux-gnu -p studio-app -p studio-designer -e features -i gpui-pre-platform@0.3.10)"
for feature in wayland x11 font-kit; do
  if ! grep -Fq "gpui-pre-platform feature \"$feature\"" <<<"$features"; then
    echo "Missing GPUI platform feature: $feature" >&2
    exit 1
  fi
done
echo "Matching GPUI 0.3.10 platform enables Wayland, X11, and macOS font-kit."
