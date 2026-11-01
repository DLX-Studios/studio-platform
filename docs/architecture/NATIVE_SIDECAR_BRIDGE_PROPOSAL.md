# Native sidecar bridge and Media Pipeline Lab proposal

**Status:** Proposal for design review
**Scope:** Native integrations for Studio apps; no implementation is implied by this document.

## Summary

Studio currently runs signed application plugins as sandboxed WebAssembly guests. The
milestone-one capability catalog contains deterministic payment and printer simulators;
the runtime does not expose general process, filesystem, media, or hardware access. This
proposal adds a separate, host-managed path for applications that need native libraries or
platform services, without giving a WASM guest arbitrary native access.

The proposed first showcase is **Media Pipeline Lab**. It demonstrates controlling a real
native media pipeline using a bundled, openly licensed fixture, while Studio owns the UI,
permissions, and lifecycle. Rust, Go, and C reference sidecars implement the same versioned
protocol. The media backend is replaceable and is not a dependency of the Studio core.

## Goals

- Let an app request a narrow, typed host capability implemented by an installed native
  sidecar.
- Keep the guest API independent of the implementation language and media library.
- Exercise request/response, asynchronous events, cancellation, process failure, version
  negotiation, and permission denial in a useful showcase.
- Keep optional native dependencies out of the base Studio runtime and apps that do not use
  them.
- Preserve the existing WASM sandbox boundary: guest code cannot choose an executable,
  command line, IPC endpoint, or arbitrary native function.

## Non-goals

- Loading arbitrary shared libraries into the Studio process.
- Replacing the WASM plugin runtime or granting WASM plugins general process-spawn access.
- Reimplementing Electron or Tauri APIs wholesale.
- Shipping a universal media codec set or promising unrestricted local-file access.
- Establishing that a separate process is, by itself, a security sandbox.
- Defining the final video-frame surface for both GPUI and web rendering in this proposal.

## Current architecture and proposed boundary

The current threat model explicitly has no child-process, media, or generic filesystem API.
Keep that true for guest imports. A signed app declares a capability such as
`media.playback`; the Studio host admits the declaration against its host-owned catalog and
app policy. Only host code can map that capability to an installed sidecar. The app calls a
typed Studio API, and the host validates and routes the request.

```text
Studio Script / WASM guest
        │ typed capability request
        ▼
Studio host: admission, consent, validation, limits, lifecycle
        │ bounded versioned IPC
        ▼
Installed native sidecar (Rust, Go, C, ...)
        │
        └── native library or OS service (for example, a media pipeline)
```

The app does not name a binary or pass a command line. The host selects a registered,
verified sidecar for an admitted capability. Initially, sidecars should be installed or
provisioned by the operator/developer, not downloaded and executed automatically from an
app bundle.

## Transport and protocol direction

Use JSON-RPC 2.0 semantics with newline-delimited JSON messages over the child process's
standard input and output for the first prototype. This avoids opening a local network port
and is available in all three target languages. Standard output is protocol-only; diagnostics
go to standard error and must be bounded/redacted by the host.

The protocol needs a fixed maximum message size, bounded in-flight requests and event queues,
request IDs, timeouts, cancellation, clean shutdown, and explicit API-version negotiation.
Unknown methods and fields fail closed. Notifications are asynchronous and are not treated as
successful command responses. A disconnected or malformed sidecar fails its pending calls;
the host can offer an explicit restart and state resynchronization policy.

Proposed initial methods:

| Method/event | Purpose |
| --- | --- |
| `initialize` | Negotiate protocol version and receive host-granted capability scope. |
| `media.openFixture` | Open a named, host-provided demo fixture; no arbitrary path in the first demo. |
| `media.play`, `media.pause`, `media.seek`, `media.setVolume` | Control the pipeline. |
| `media.getState` | Return playback state, duration, position, and available tracks. |
| `media.stateChanged` | Publish bounded playback and track changes. |
| `media.error` | Report a typed, non-sensitive media failure. |
| `shutdown` | Stop work and release resources within a host deadline. |

Names and schemas above are illustrative pending protocol design. Media events should be
coalesced or rate-limited; the guest must not be able to flood the UI queue with position
updates.

## Permissions and security

- Capability declarations request access; they never grant it. Host policy and explicit
  consent decide whether a capability is available.
- The initial showcase grants access only to its bundled fixture and playback controls. It
  does not grant arbitrary file reads, network access, or process spawning to the guest.
- Bind each sidecar session to the admitted app identity and runtime instance. Do not reuse
  handles or sessions across app restarts.
- Validate every message against a closed, size-bounded schema before dispatch or state
  mutation. Do not log message payloads by default.
- Apply startup, request, event-rate, memory/CPU where available, and shutdown limits. Treat
  a crash, malformed frame, protocol mismatch, or timeout as a sidecar failure and clean up
  its session.
- Process separation limits crash propagation but does not contain the sidecar's OS
  privileges. Trust, signing, installation provenance, OS sandboxing, and permission policy
  need separate design before third-party sidecars can be treated as untrusted.
- Do not expose unrestricted shell execution, arbitrary paths, or executable selection to
  Studio Script.

## Showcase: Media Pipeline Lab

The demo is a small media control room: bundled sample video/audio, play and pause, seeking,
volume, track/subtitle metadata, a live event/history panel, and a sidecar inspector showing
language, protocol version, health, event rate, and typed errors. Developers can select the
Rust, Go, or C implementation and use the same UI and contract.

The showcase should use a small fixture with a license that permits redistribution, and keep
its source and license notice in the repository. The selected media engine, bindings, optional
plugins, codecs, and fixture each require their own license and redistribution review. Do not
make a blanket “license-free” claim for GStreamer, FFmpeg, VLC, or a codec bundle. The base
Studio runtime should not link the media engine.

The three language implementations prove protocol portability, not identical media backend
behavior. Use a shared conformance suite and report backend-specific capabilities explicitly.
Begin with a real media library in the reference implementation; add Go and C versions only
after the protocol boundary is stable. Each implementation must pass the same command, event,
error, and lifecycle cases.

### Rendering constraint

Native decoding and playback control do not settle how video frames reach the UI. A GPUI
native surface and a browser renderer have different media and compositing paths. Keep the
protocol's control plane renderer-neutral, and run a separate technical spike to compare:

1. Host-owned native video surface for GPUI and browser-native video for web, with equivalent
   Studio controls; or
2. A host media service that emits frames through a defined renderer interface, if acceptable
   for performance and complexity.

Do not promise that a GPUI plugin or surface automatically works in the web renderer.

## Licensing and packaging

Keep sidecar packages separate from the Studio runtime and application bundle. The showcase
must list exact dependency versions, licenses, enabled plugins/codecs, and fixture attribution
in `THIRD_PARTY_NOTICES.md` or a scoped notice file. Review the actual build configuration and
redistribution obligations before shipping binaries. Prefer an optional backend that users can
install independently when redistribution terms or codecs vary by platform.

## Staged delivery

1. **Protocol spike:** specify framing, schema, version negotiation, limits, cancellation,
   event backpressure, and failure behavior. Test a tiny echo sidecar in Rust, Go, and C.
2. **Host bridge prototype:** host-managed process registration and launch, capability
   admission, bounded IPC, typed errors, lifecycle, and a non-UI conformance suite. Keep
   arbitrary guest process access unavailable.
3. **Media reference:** implement the Rust sidecar and Media Pipeline Lab against one chosen
   backend and redistributable fixture. Complete the video-surface spike before committing to
   a cross-renderer playback promise.
4. **Language parity:** implement Go and C sidecars and run the same conformance suite. Record
   optional backend features and platform limitations.
5. **Trust and distribution:** design signing, installation, update/revocation, OS sandboxing,
   and permissions before supporting third-party sidecars in production.

## Acceptance criteria for a first prototype

- A WASM guest can invoke only an admitted, named host capability; it cannot select or spawn a
  process.
- The host can launch a registered sidecar and negotiate a supported protocol version.
- Rust, Go, and C echo implementations pass the same request/response, event, malformed
  message, timeout, cancellation, disconnect, and shutdown cases.
- Message size, concurrent work, and event queues have explicit limits.
- A sidecar crash does not crash Studio; pending requests terminate with a typed host error.
- Denied or undeclared capabilities never launch a sidecar.
- Media backend and fixture licenses are recorded before the showcase is redistributed.

## Open decisions

- Should the first trusted sidecars be per-user installed applications, app-vendored binaries,
  or host-provisioned services? The prototype should avoid app-controlled executable paths.
- What OS process sandboxing is available and supportable on each target platform?
- Which media backend and minimal codec set meet the showcase goals without unacceptable
  packaging or licensing costs?
- What is the supported video rendering contract across GPUI desktop and the web renderer?
- Which capability consent UX applies to signed first-party apps versus third-party apps?
- Should protocol messages use newline-delimited JSON for the first version or a length-prefixed
  encoding if binary media/control data is later required? Media bytes should not travel over
  this control protocol in the initial design.
