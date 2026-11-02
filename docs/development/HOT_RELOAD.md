# Native development hot reload

`studio dev <project-or-example>` opens its own GPUI window. It does not launch `studio-app` and
it never creates a `.studio` archive. `studio build` retains the production Wasm/package pipeline.
`studio preview <project-or-bundle-path>` also uses the CLI's native development host.

The code is split across these boundaries:

- `studio-cli::dev`: watcher, source hashes, compiler cache, IR interpreter integration,
  development window, update commits and the local restart endpoint.
- `studio-runtime`: shared manifest admission, provider services, Wasm instantiation and native
  guest surface. The `development` feature enables loose-artifact admission and SDK state transfer.
- `studio-components::native_view`: shared renderer and stable widget entities.
- `studio-app`: production entry point. Only signed `--bundle <absolute-path>` launches are
  accepted. Its normal dependency graph does not include Studio Script/rsvelte, Designer
  persistence, or the reload server. Cargo may unify development features when both executables
  are built together; production admission still rejects development exports and unsigned input.

## Changes and cache granularity

A single background worker hashes file contents and identities, including project sources,
relative external imports, SDK sources, compiler configuration and toolchain metadata. Additions
and deletions invalidate the input set. Generated output, `build/`, `.git/`, and `node_modules/`
are excluded from the project scan. Package/lock metadata participates in the fingerprint;
arbitrary edits inside third-party `node_modules` are not watched.

Each file has its own hash. The current compiler's replaceable units are a Studio Script entry
module and a linked Wasm application module. The existing production compiler supports a single
Studio entry (`app.studio`, or one component/route source). This change does not add a new
multi-component language composer or a dynamic Wasm linking ABI. A change to a dependency of a
linked Wasm module recompiles that unit; it does not replace individual machine-code functions.

Studio Script compiles directly to typed IR and executes through the Rust evaluator. No ASC,
Wasm or ZIP generation is needed. Compatible stable state slots retain live values; retyped
slots initialize from the new source. The replacement tree must pass protocol and native catalog
admission before the live module changes.

AssemblyScript compiles through a generated development entry with a debug compiler target and
SDK hot state exports. Modules are cached under `build/.studio-dev/cache/<fingerprint>.wasm`.
Completed compiler output is published atomically so interrupted compiles cannot poison a cache key.
The cache key includes development compiler implementation, SDK source, compiler metadata,
configuration, relative imports and requested Wasm memory limits. Cache hits still pass Wasm
and manifest policy before execution. Asset-only edits reuse the current guest and refresh the
admitted bytes. Manifest changes re-admit a candidate using cached code where possible.

The worker validates/instantiates a candidate away from the UI thread. The UI thread captures
current hot state, restores it before candidate initialization, validates its initial tree and
then replaces the surface while retaining the window and native widget entities. An update is
acknowledged only after that commit. Syntax, policy or mount errors keep the last committed
surface and watcher running. Changes during compilation produce a trailing update.

## Wasm module state

Ordinary Wasm modules reload with fresh guest memory. A live Wasm module cannot safely preserve
raw linear memory across arbitrary recompilation: offsets, allocator state and function-table
indices may change. Register explicit module state through the development SDK instead:

```ts
import { registerHotState } from "../../../sdk/assemblyscript/assembly/dev";

let count: i32 = 0;
registerHotState(
  "counter",
  () => count.toString(),
  (document: string): void => { count = <i32>parseInt(document); },
);
```

Register at module scope with a unique, stable id. The reader must only read state. The writer
runs before `studio_init`, so the new initial UI should render the restored values. Complex
modules should return a versioned document and restore only compatible fields. Newly registered
ids use their initial values; removed ids are ignored. The SDK frames each module's document in
a versioned envelope, and the host limits the entire snapshot to 64 KiB. Hooks use normal guest
fuel and deadline limits. A trapping hook is a guest failure; it cannot provide recoverable state.

The CLI injects exports `studio_dev_snapshot` and `studio_dev_restore` only into development
compilation. Registration is removed when `STUDIO_DEV` is undefined. Normal production module
validation continues to require exactly the original five ABI exports and rejects dev exports.
No production ABI import is added.

`studio restart-session <project>` deliberately initializes fresh state using the current source.
The legacy `--port` option remains accepted for CLI compatibility; native updates use an
in-process channel and the project's local restart socket, not an HTTP server. Changes to native
Rust host code itself still require rebuilding the executable.
