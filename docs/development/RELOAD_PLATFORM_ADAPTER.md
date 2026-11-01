# Reload platform adapter — 2026-11-01

Studio's local reload IPC now uses `ReloadListener` and `ReloadStream` from
`studio_protocol::reload`. Unix targets re-export the standard-library domain sockets;
Windows targets use the Windows-only `uds_windows = 1.2.1` dependency (MIT).
The Runtime, host re-exports, CLI, and reload integration tests use these shared types.

The adapter keeps newline-delimited JSON, `studio-reload/1`, request correlation, deadlines,
stream cloning, listener cancellation polling, and session revision semantics. It opens no
TCP endpoint and grants no additional guest capability. Existing endpoint binding and stale-file
handling are preserved. The channel remains development-only, with the existing trust model.

Windows uses filesystem AF_UNIX (Windows 10 version 1803 or later). It always computes a short,
stable hashed endpoint in the system temporary directory, avoiding extended-length checkout
paths in Winsock's UTF-8 address. Unix retains the existing in-build-directory endpoint and
hashed fallback for long paths. Peers must pass the same project build directory when computing
the endpoint. Runtime discovery now includes the native executable suffix, and the CLI's Unix
resource-limit helper has a no-op implementation on other platforms. The shell-based watcher
cancellation fixture is explicitly Unix-only; transport tests run on both platforms.

## Verification

- Reproduced the missing Windows transport types with a failing cross-compilation check before
  adding the Windows adapter.
- `cargo check --locked -p studio-protocol -p studio-cli --target x86_64-pc-windows-gnu --tests`
  passed, including the CLI reload test targets. This checks Windows code without linking or
  running a Windows executable on Linux.
- `cargo test --locked -p studio-protocol --lib reload::tests` passed on Linux (8 tests),
  including real local request/reply with a cloned writer, missing-endpoint errors, nonblocking
  accept, read timeout, protocol validation, and deterministic path selection.
- Integration checkpoint: `cargo fmt --all -- --check`,
  `cargo test --locked --workspace` (1,290 passed, 2 intentionally ignored), and
  `cargo clippy --locked --workspace --all-targets -- -D warnings` passed sequentially.
  The Linux link used the existing native-library shim directory via `LIBRARY_PATH`.
- Foundation/documentation tests passed (27 tests); the native platform feature check passed.
- CI now runs the same transport tests and checks CLI test targets natively on Windows and
  macOS. Those jobs provide runtime evidence after the change is pushed; configuring them
  is not evidence that they passed.

This removes the direct Unix socket dependency from the reload callers. It does not claim a
complete Windows build or release certification for GPUI, the host database, OS credential
services, or application packaging. Full application validation on Windows and macOS remains
required. The dependency alignment evidence is recorded separately in
[GPUI alignment](GPUI_ALIGNMENT.md).

API references: [uds_windows](https://docs.rs/uds_windows/1.2.1/uds_windows/) and
[Microsoft AF_UNIX overview](https://devblogs.microsoft.com/commandline/af_unix-comes-to-windows/).
