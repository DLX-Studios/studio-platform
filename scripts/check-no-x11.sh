#!/usr/bin/env bash
set -euo pipefail

binary="${1:-target/release/studio-app}"

if [[ ! -f "$binary" ]]; then
  echo "Studio binary not found: $binary" >&2
  exit 2
fi

if ldd "$binary" | grep -Eiq 'libX11|libxcb|libXau|libXdmcp'; then
  echo "X11/XCB linkage detected in $binary" >&2
  ldd "$binary" | grep -Ei 'libX11|libxcb|libXau|libXdmcp' >&2
  exit 1
fi

if readelf -d "$binary" | grep -Eiq 'libX11|libxcb|libXau|libXdmcp'; then
  echo "X11/XCB ELF dependency detected in $binary" >&2
  readelf -d "$binary" | grep -Ei 'libX11|libxcb|libXau|libXdmcp' >&2
  exit 1
fi

echo "No X11/XCB dynamic linkage detected in $binary"
