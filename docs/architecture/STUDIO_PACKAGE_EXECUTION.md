# Studio package execution direction

This note records the proposed cross-project architecture. Existing Studio Runtime still uses
Wasmtime for plugin guests, and the current Orbit deployment path is container-backed. The work
below is a target, not a claim that those systems have been replaced.

## Source and artifact

- `.studio` is the source extension for Studio UI/components.
- `.studio.ts` is the preferred source extension for supported TypeScript logic, commands, and
  server modules. It compiles to Studio IR; arbitrary JavaScript execution is not implied.
- `.studiopkg` is the preferred versioned bundle extension. A package may carry binary IR, assets,
  declarations, client/server entry points, and capability requirements. The `.op` suffix is
  unresolved; do not create a second package format without a proven compatibility need.
- Package metadata and IR encoding require explicit schema/version negotiation, bounded decoding,
  deterministic builds, signatures or provenance, and backward-compatibility rules.

## Runtime and hosts

Studio Runtime owns validation and execution semantics for UI and server logic. Hosts provide
different capabilities over one typed contract:

- Native apps render package UI through GPUI and host-owned system services.
- Browser apps render through a supported browser host; they call Console/Orbit server APIs and
  never use Unix sockets directly.
- Orbit runs a headless Studio host for server entry points and plugin commands.

The existing `studio-script` evaluator and serialized `StudioModule` are useful starting points;
they do not yet constitute the complete package loader, multi-host ABI, or production plugin
execution contract.

## Capabilities and process isolation

Packages declare requested capabilities, while installation policy grants them. A runtime checks
each call against the authenticated installation and workspace. Filesystem, network, database,
secrets, process control, and hardware operations are host services, never arbitrary guest syscalls.

On Linux, Orbit's process launcher must start guests with a dedicated unprivileged identity, no
ambient device or filesystem access, restricted network/syscalls, resource limits, and no inherited
privileged descriptors. Per-workload Unix sockets provide authenticated local IPC; Orbit's
agent/edge transport carries requests between machines. Plugins at different trust levels start in
separate processes. Native libraries execute with process authority and therefore require a
separately isolated helper or an explicit trusted-host classification.

The process launcher must fail closed if required isolation controls are unavailable. Kernel
isolation reduces the impact of runtime bugs; it does not remove the need for hardened parsing,
execution budgets, patch review, and adversarial tests.

## Repository responsibilities

- Studio Platform: source syntax, compiler, IR, package format, signing/provenance rules, runtime,
  host APIs, and native/browser/headless acceptance.
- Studio Console: Designer, customer identity and permissions, marketplace, immutable source
  revisions, and reproducible build requests.
- Orbit: build worker, process placement/supervision, scoped socket credentials, health, scaling,
  routing, storage, and deployment lifecycle.

## Milestones

1. Freeze source subset, compiler output, package metadata, IR encoding, commands, capabilities,
   and compatibility policy.
2. Build a deterministic `.studiopkg` loader and validator with native and headless conformance
   tests; define the browser host separately with browser-appropriate IPC.
3. Implement a restricted headless host and command broker without replacing current deployment.
4. Prototype Orbit direct-process supervision using one Linux workload and adversarial isolation
   tests. Compare lifecycle, recovery, rolling upgrade, and resource accounting with the existing
   provider before selecting rollout criteria.
5. Add immutable Console source revisions and build snapshots, then prove the complete path with
   a Studio-authored UI and an Orbit-hosted Resend command.
6. Migrate other integrations and deployment types only after that proof and compatibility plan.

## Unresolved decisions

- Whether package needs multiple file extensions or `.studiopkg` suffices.
- How much TypeScript syntax `.studio.ts` supports and which runtime features compile to IR.
- Shared state and reactive update semantics across UI and server entry points.
- Package signature/provenance trust, upgrades, rollback, and old-runtime compatibility.
- Source retention and artifact lifecycle for customer Console workspaces.
- Orbit's accepted Linux baseline, sandbox controls, remote IPC, failure recovery, and process
  migration guarantees.
- The `studio-site` checkout includes both public Website and Console; settle the Website destination
  before physically renaming the Console project/repository to `studio-console`.
