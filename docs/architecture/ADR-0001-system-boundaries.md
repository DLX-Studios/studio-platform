# ADR-0001: Studio System Boundaries

- Status: Accepted
- Date: 2026-08-03

## Context

Studio must run untrusted business logic while retaining control of native rendering, secrets, hardware, networking, and system resources.

## Decision

The application is split into two trust domains:

1. The Rust host owns GPUI rendering, navigation, bundle verification, capabilities, secrets, persistence, network clients, and hardware adapters.
2. AssemblyScript plugins execute in Wasmtime without WASI. They receive events and emit only versioned declarative messages through one host import.

The guest never receives a native widget reference, file descriptor, socket, raw secret, host pointer, or arbitrary system-call interface.

All plugin-visible routes, components, properties, events, lifecycle messages, action operations,
and error families are closed protocol-v1 types. Unknown values fail before mutation. UI patches,
navigation commands, action admission, and confirmation snapshots are independently staged and
committed atomically by their owning host subsystem.

Sensitive controls and trusted confirmation/failure/print-preview overlays are outside the plugin
tree. Plugins observe readiness and stable result codes, not raw input or provider/device channels.
Opaque references are exact-principal, purpose, session, expiry, and use-count capabilities—not
credentials and not transferable bearer values across instances.

The supported platform boundary is native Wayland only. Missing endpoints are rejected without an
X11 fallback. Compositor disconnect performs ordered terminal cleanup and exits; restoration would
risk reattaching stale native state and sensitive actions, so it is intentionally unsupported.

## Consequences

- The protocol is a security boundary and must be validated before state mutation.
- Native components are implementation details behind Studio schema types.
- Host actions are asynchronous requests checked against a signed principal.
- Web support, if added later, implements the same protocol rather than weakening the native boundary.
- Payment and printing remain deterministic offline simulators until separate specifications add
  audited provider/hardware capabilities.
- Trapped instances require explicit manual restart with a fresh identity and empty plugin state.
- Protocol stability improves auditability but requires generated-schema and traceability gates for
  every version change.
