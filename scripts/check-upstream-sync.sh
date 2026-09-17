#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
component_head="$(git ls-remote https://github.com/longbridge/gpui-component.git refs/heads/main | awk '{print $1}')"
pre_latest="$(curl -s --max-time 60 https://index.crates.io/gp/ui/gpui-pre | grep -o '"vers":"[^"]*"' | tail -1 | cut -d'"' -f4)"

configured_component="$(sed -n 's/.*Revision: `\([0-9a-f]\{40\}\)`.*/\1/p' "$root_dir/vendor/gpui-component/UPSTREAM.md" | head -1)"
configured_pre="$(sed -n 's/^gpui = { version = "=\([0-9.]*\)", package = "gpui-pre".*/\1/p' "$root_dir/Cargo.toml" | head -1)"

printf 'gpui-component main:  %s\n' "$component_head"
printf 'Studio component pin: %s\n' "$configured_component"
printf 'gpui-pre latest:      %s\n' "$pre_latest"
printf 'Studio gpui-pre pin:  %s\n' "$configured_pre"

if [[ "$component_head" != "$configured_component" || "$pre_latest" != "$configured_pre" ]]; then
  echo "Upstream revisions have advanced; run the reviewed synchronization workflow." >&2
  exit 2
fi

echo "Upstream pins are current."
