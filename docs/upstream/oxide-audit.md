# Oxide Final Extraction Audit

Upstream: https://github.com/niklabh/oxide
Audited revision: `29cd89882465d6ebfe00af2ada6f89951581c580`
License: Apache-2.0
Final audit date: 2026-08-04

Oxide informed the initial review of Wasmtime engine configuration, manifest permissions, and
guest-memory copying. Studio did not use Oxide as a repository base and no Oxide source text was
copied. Every shipping implementation below was independently written against Studio's closed
protocol and verified by Studio tests.

| Studio area | Oxide reference reviewed | Studio-specific result | Evidence | Source status |
| --- | --- | --- | --- | --- |
| `studio-wasm` engine/runtime | `oxide-browser/src/engine.rs`, `runtime.rs` | Wasmtime with fuel, epochs, store limiters, one import, no WASI | `module_policy`, `runtime_limits` | Independent implementation |
| Guest memory/emit | `oxide-browser/src/capabilities.rs` | copy-before-validation, checked pointer/length, bounded deferred queue | `guest_memory` | Independent implementation |
| Manifest/capabilities | `manifest.rs`, `permissions.rs` | closed v1 manifest and two simulator capabilities | `manifest_v1`, `principal_policy` | Independent implementation |

Excluded permanently from milestone one: canvas/frame loop, browser navigation, Forge, Rust guest
SDK, HTTP/WebSocket/WebRTC, media/capture/MIDI, clipboard, filesystem/downloads, WebGPU, workers,
child modules, and persistent storage. Because no source was extracted, there is no patch-level
Oxide attribution obligation beyond this architectural reference and the third-party notice.
