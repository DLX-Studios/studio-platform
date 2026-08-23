#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
zed_head="$(git ls-remote https://github.com/zed-industries/zed.git refs/heads/main | awk '{print $1}')"
component_head="$(git ls-remote https://github.com/longbridge/gpui-component.git refs/heads/main | awk '{print $1}')"

configured_zed="$(sed -n 's/.*rev = "\([0-9a-f]\{40\}\)".*/\1/p' "$root_dir/Cargo.toml" | head -1)"
configured_component="$(sed -n 's/.*Revision: `\([0-9a-f]\{40\}\)`.*/\1/p' "$root_dir/vendor/gpui-component/UPSTREAM.md" | head -1)"

printf 'Zed main:             %s\n' "$zed_head"
printf 'Studio GPUI pin:      %s\n' "$configured_zed"
printf 'gpui-component main:  %s\n' "$component_head"
printf 'Studio component pin: %s\n' "$configured_component"

if [[ "$zed_head" != "$configured_zed" || "$component_head" != "$configured_component" ]]; then
  echo "Upstream revisions have advanced; run the reviewed synchronization workflow." >&2
  exit 2
fi

echo "Upstream pins are current."
