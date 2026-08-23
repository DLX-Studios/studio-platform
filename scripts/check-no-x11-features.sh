#!/usr/bin/env bash
set -euo pipefail

feature_tree="$(cargo tree --locked -p studio-app -p studio-components -e features,no-dev)"
forbidden='x11rb|x11-clipboard|zed-xim|as-raw-xcb|x11-dl|winit feature "x11"|gpui_linux feature "x11"|gpui feature "x11"'

if grep -Eiq "$forbidden" <<<"$feature_tree"; then
  echo "X11 capability detected in a shipping Studio Cargo feature graph" >&2
  grep -Ei "$forbidden" <<<"$feature_tree" >&2
  exit 1
fi

echo "No X11 capability detected in the Studio app/component Cargo feature graphs"
