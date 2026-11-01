#!/usr/bin/env bash
set -euo pipefail
root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
python3 - "$root_dir" <<'PYTHON'
import json
from pathlib import Path
import sys
import tomllib
import urllib.request

root = Path(sys.argv[1])
packages = tomllib.loads((root / "Cargo.lock").read_text())["package"]
current = True
for name in ("gpui-kit", "gpui-pre", "gpui-pre-platform"):
    locked = {p["version"] for p in packages if p["name"] == name}
    request = urllib.request.Request(
        "https://crates.io/api/v1/crates/" + name,
        headers={"User-Agent": "studio-platform-upstream-check"},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        latest = json.load(response)["crate"]["max_stable_version"]
    print(f"{name}: locked={','.join(sorted(locked))}; latest={latest}")
    current &= locked == {latest}
if not current:
    sys.exit("Published versions differ; review and update Kit and its matching GPUI family together.")
PYTHON
