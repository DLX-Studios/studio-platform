#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
kit_head="$(git ls-remote https://github.com/longbridge/gpui-kit.git refs/heads/main | awk '{print $1}')"
pre_latest="$(curl -s --max-time 60 https://index.crates.io/gp/ui/gpui-pre | grep -o '"vers":"[^"]*"' | tail -1 | cut -d'"' -f4)"

configured_kit="$(sed -n 's/.*Revision: `\([0-9a-f]\{40\}\)`.*/\1/p' "$root_dir/vendor/gpui-kit/UPSTREAM.md" | head -1)"
configured_pre="$(sed -n 's/^gpui = { version = "=\([0-9.]*\)", package = "gpui-pre".*/\1/p' "$root_dir/Cargo.toml" | head -1)"

printf 'gpui-kit main:       %s\n' "$kit_head"
printf 'Studio gpui-kit pin: %s\n' "$configured_kit"
printf 'gpui-pre latest:      %s\n' "$pre_latest"
printf 'Studio gpui-pre pin:  %s\n' "$configured_pre"

if [[ "$kit_head" != "$configured_kit" || "$pre_latest" != "$configured_pre" ]]; then
  echo "Upstream revisions have advanced; run the reviewed synchronization workflow." >&2
  exit 2
fi

echo "Upstream pins are current."
