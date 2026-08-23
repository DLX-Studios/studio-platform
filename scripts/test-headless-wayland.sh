#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
studio_binary="${STUDIO_APP_BINARY:-${repository_root}/target/debug/studio-app}"
compositor_config="${repository_root}/tests/platform/sway-headless.conf"

if ! grep -Eq '^xwayland[[:space:]]+disable$' "${compositor_config}"; then
  echo "headless compositor must explicitly disable XWayland" >&2
  exit 1
fi

if ! command -v sway >/dev/null 2>&1; then
  echo "sway is required for the native headless Wayland launch test" >&2
  exit 1
fi

if ! command -v dbus-run-session >/dev/null 2>&1; then
  echo "dbus-run-session is required for the native headless Wayland launch test" >&2
  exit 1
fi

if ! command -v setsid >/dev/null 2>&1; then
  echo "setsid is required for isolated headless compositor cleanup" >&2
  exit 1
fi

if [[ ! -x "${studio_binary}" ]]; then
  cargo build --locked -p studio-app
fi

set +e
unsupported_output="$(
  env -u DISPLAY -u WAYLAND_DISPLAY -u WAYLAND_SOCKET \
    "${studio_binary}" 2>&1
)"
unsupported_status=$?
set -e

if [[ ${unsupported_status} -ne 2 ]]; then
  echo "Studio exited with ${unsupported_status}, not 2, without a Wayland endpoint" >&2
  echo "${unsupported_output}" >&2
  exit 1
fi

if [[ "${unsupported_output}" != *"Studio requires a native Wayland session"* ]]; then
  echo "Studio did not report its native Wayland requirement" >&2
  echo "${unsupported_output}" >&2
  exit 1
fi

echo "missing Wayland endpoint rejection: ok"

runtime_dir="$(mktemp -d "${TMPDIR:-/tmp}/studio-wayland.XXXXXX")"
chmod 700 "${runtime_dir}"
compositor_pid=""
studio_pid=""

cleanup() {
  if [[ -n "${studio_pid}" ]] && kill -0 "${studio_pid}" 2>/dev/null; then
    kill -TERM "${studio_pid}" 2>/dev/null || true
    wait "${studio_pid}" 2>/dev/null || true
  fi
  if [[ -n "${compositor_pid}" ]] && kill -0 "${compositor_pid}" 2>/dev/null; then
    kill -TERM -- "-${compositor_pid}" 2>/dev/null || true
    wait "${compositor_pid}" 2>/dev/null || true
  fi
  case "${runtime_dir}" in
    "${TMPDIR:-/tmp}"/studio-wayland.*) rm -r -- "${runtime_dir}" ;;
  esac
}
trap cleanup EXIT INT TERM

export XDG_RUNTIME_DIR="${runtime_dir}"
export WLR_BACKENDS="headless"
export WLR_LIBINPUT_NO_DEVICES="1"
unset DISPLAY WAYLAND_DISPLAY WAYLAND_SOCKET
xwayland_before="$(pgrep -x Xwayland 2>/dev/null || true)"

setsid dbus-run-session -- sway -c "${compositor_config}" \
  >"${runtime_dir}/sway.log" 2>&1 &
compositor_pid=$!

wayland_socket=""
for _ in {1..100}; do
  for candidate in "${runtime_dir}"/wayland-*; do
    if [[ -S "${candidate}" ]]; then
      wayland_socket="${candidate}"
      break 2
    fi
  done
  if ! kill -0 "${compositor_pid}" 2>/dev/null; then
    echo "headless Sway exited before creating its Wayland socket" >&2
    sed -n '1,160p' "${runtime_dir}/sway.log" >&2
    exit 1
  fi
  sleep 0.05
done

if [[ -z "${wayland_socket}" ]]; then
  echo "headless Sway did not create a native Wayland socket" >&2
  sed -n '1,160p' "${runtime_dir}/sway.log" >&2
  exit 1
fi

export WAYLAND_DISPLAY="${wayland_socket##*/}"

"${studio_binary}" >"${runtime_dir}/studio.log" 2>&1 &
studio_pid=$!

for _ in {1..20}; do
  if ! kill -0 "${studio_pid}" 2>/dev/null; then
    wait "${studio_pid}" || studio_status=$?
    echo "Studio exited during native headless Wayland startup (${studio_status:-0})" >&2
    sed -n '1,160p' "${runtime_dir}/studio.log" >&2
    exit 1
  fi
  sleep 0.05
done

if ! kill -0 "${compositor_pid}" 2>/dev/null; then
  echo "headless Sway exited while Studio was running" >&2
  sed -n '1,160p' "${runtime_dir}/sway.log" >&2
  exit 1
fi

xwayland_after="$(pgrep -x Xwayland 2>/dev/null || true)"
for candidate in ${xwayland_after}; do
  if ! grep -qx "${candidate}" <<<"${xwayland_before}"; then
    echo "headless acceptance started an XWayland process (${candidate})" >&2
    exit 1
  fi
done

echo "native headless Wayland launch with no XWayland process: ok"
