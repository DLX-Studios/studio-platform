# Runtime build variants

`studio-runtime` contains the native host library. `studio-app` is its runnable
window host. Both enable the `services` feature by default, preserving the full
runtime used by current callers.

## Core build

```sh
cargo build --locked --release -p studio-app --no-default-features
```

The core variant retains the existing GPUI renderer and components, Wasmtime
execution and budgets, reactive patches, file routes, timers, asset loading,
signed bundle verification, and host networking primitives. It does not change
the app language, the guest ABI, image resolution, or rendering backend.

It excludes the runtime's database/migration execution dependency, OAuth and
GitHub service implementations, checkout/payment surfaces, print preview, and
the service-specific shutdown coordinator. The package and security crates
remain because the core still validates signed packages and enforces guest
boundaries. A core build does not populate the built-in provider registry.

Bundles declaring service capabilities, integrations, secrets or provider routes
are rejected by the core host. Bundles requiring migrations are also rejected;
missing services are never treated as successful no-ops. Unexpected guest host
actions receive a non-retryable `capability_denied` response.

This is an experiment in a smaller Studio core, not an implementation of Shell's
JavaScript engine, view cache, native-module API, or plugin loader. Services are
compile-time options at present, not independently installable native plugins.

## Compact executable

```sh
cargo build --locked --profile compact -p studio-app --no-default-features
```

The opt-in `compact` profile adds size optimization, thin LTO, one codegen unit,
and symbol stripping. It keeps unwinding behavior unchanged. Its executable is
in the configured Cargo target's `compact/studio-app`, separate from
`release/studio-app`. Normal release settings are unchanged.

The standard-profile core build isolates service-removal effects. The compact
build also changes optimization and may trade execution speed for file size.
Compare both against a full build with the same symbol-stripping treatment.
Binary size is not a prediction of RSS or PSS; measure with the same app bundle,
GPU, window size and sampling interval. Do not use software rendering for these
measurements.

The core is selected only when all dependency paths disable runtime defaults.
Build `-p studio-app`, rather than the whole workspace, for this experiment:
Cargo feature unification can otherwise re-enable services through another host.

## Focused verification

```sh
cargo test --locked --release -p studio-runtime --no-default-features --lib --test core_launch
cargo test --locked --release -p studio-runtime --lib --test core_launch
```

These exercise guest execution, patches, navigation, timer lifecycle, signed
bundle admission, and unavailable-service rejection without creating a renderer.
Use a hardware GPU session for native launch checks. Service-dependent app integration
targets require `services`; they remain enabled in the default configuration.
