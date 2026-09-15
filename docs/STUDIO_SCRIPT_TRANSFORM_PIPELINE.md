# Studio Script Transform Pipeline

**Rust + rsvelte + Studio IR + ASC — hand-off-ready**

## Status

This document defines the planned `.studio` source pipeline for development and production.

The architecture does **not** embed QuickJS. Development uses `rsvelte` as the Svelte-compatible source frontend, lowers its AST into a typed Studio-specific intermediate representation, and executes that IR in the Rust development runtime. Production lowers the same IR into AssemblyScript-compatible modules and invokes ASC to produce Wasm.

Current implementation status:

- The AssemblyScript SDK, generated bindings, signals, ASC invocation, Wasmtime sandbox, and native GPUI host exist.
- `crates/studio-script` exists as the Studio-owned compiler boundary and currently implements source preparation and `<script>` extraction.
- The `rsvelte` adapter, Studio IR, Rust IR evaluator, IR hot swap, and AssemblyScript lowering backend remain planned work.
- QuickJS is not implemented and is no longer part of this pipeline.

---

## 1. Goal

```text
                         ┌─ development ─────────────────────────────┐
ProductCard.studio       │                                           │
        │                ▼                                           │
        ├─ rsvelte AST → typed Studio IR → Rust dev runtime → GPUI ──┘
        │                                  identity-preserving swap
        │
        └─ production → typed Studio IR → AssemblyScript → ASC → Wasm
                                                               │
                                                               ▼
                                                       Studio host → GPUI
```

The source is parsed and validated against one portable Studio language contract. Development and production share the same typed IR but use different consumers:

- Development interprets IR directly in Rust for fast updates without rebuilding Wasm after every edit.
- Production generates AssemblyScript-compatible modules and compiles them to Wasm.
- A future web host can load the same Wasm. It may require small JavaScript loader or browser-binding glue, but it does not require QuickJS.

Generated JavaScript is **not** passed into ASC. ASC consumes AssemblyScript-compatible source generated from the typed Studio IR.

---

## 2. Example source file

```svelte
<!-- components/ProductCard.studio -->
<script lang="ts">
  let { name, price, available = true } = $props();
  let quantity = $state(1);

  function addToOrder() {
    emit("add-to-order", { productId: name, quantity });
  }
</script>

<Card id="product-card" padding={12}>
  <Text id="product-name" typographyRole="label">{name}</Text>
  <Text id="product-price">{formatMoney(price)}</Text>

  {#if available}
    <Button id="add-button" disabled={!available} onclick={addToOrder}>
      Add to cart ({quantity})
    </Button>
  {:else}
    <Text>Unavailable</Text>
  {/if}
</Card>
```

Studio Script deliberately looks like Svelte, but component tags refer to the closed Studio component catalog rather than HTML elements.

---

## 3. AST versus Studio IR

AST means **Abstract Syntax Tree**. It represents what the developer wrote, including Svelte syntax, source ordering, and source spans. It is the right shape for parsing, diagnostics, formatting, projection, and editor tooling.

IR means **Intermediate Representation**. It represents what the Studio runtime needs to execute, independent of Svelte syntax and independent of any one output target.

For example, this source:

```svelte
{#if available}
  <Button onclick={addToOrder}>Add</Button>
{:else}
  <Text>Unavailable</Text>
{/if}
```

is normalized into an IR shape resembling:

```text
If {
    condition: ReadProp("available"),
    consequent: [
        Component {
            kind: Button,
            props: { onclick: Handler("addToOrder") },
            children: [Text("Add")],
        },
    ],
    alternate: [
        Component {
            kind: Text,
            children: [Text("Unavailable")],
        },
    ],
}
```

The `rsvelte` AST must not leak through the rest of the application. Only the adapter in `studio-script` depends on its concrete API. This keeps Studio runtime code stable if `rsvelte` changes or is replaced.

---

## 4. High-level development flow

```text
Rust file watcher
    │ detects a changed .studio source
    ▼
studio_script::prepare(path, source)
    │
    ├─ establish module identity and source fingerprint
    ├─ extract/validate the supported script block convention
    └─ hand source to the pinned rsvelte adapter
            │
            ▼
rsvelte frontend
    ├─ parse Svelte markup and script syntax
    ├─ report syntax diagnostics and source spans
    ├─ expose rune/component facts
    └─ optionally provide projection data for tooling
            │
            ▼
Studio AST adapter and validator
    ├─ resolve Studio SDK imports and component names
    ├─ validate props, events, state, expressions, and control flow
    ├─ reject unsupported JavaScript/TypeScript behavior
    └─ lower the source AST into typed Studio IR
            │
            ▼
Rust Studio IR runtime
    ├─ evaluate supported expressions and handlers
    ├─ update retained component nodes
    ├─ preserve compatible state slots and widget identity
    └─ atomically publish or reject the replacement component
```

No JavaScript module is evaluated during this flow, and no Wasm module is rebuilt for each source edit.

---

## 5. The portable Studio language subset

Removing QuickJS means the development runtime cannot accept arbitrary JavaScript merely because it parses as TypeScript. Every executable construct must have defined Studio semantics, a typed IR representation, a Rust evaluator implementation, and an AssemblyScript lowering rule.

The initial portable subset should include:

- Explicitly typed or unambiguously inferred props, locals, parameters, and return values.
- Serializable primitives, arrays, maps, records, and closed Studio SDK types.
- `$props`, `$state`, `$derived`, and the approved effect/lifecycle APIs.
- Studio component construction, conditionals, keyed iteration, interpolation, and composition.
- Typed Studio events and calls to explicitly approved pure SDK functions.
- Static imports resolved through the Studio module graph.

It should reject at compile time:

- Browser DOM APIs and Node/Bun filesystem or process APIs.
- `any`, runtime property creation, prototype mutation, reflection, and `eval`.
- Unrestricted dynamic imports and dynamically selected functions or classes.
- Unsupported closures, promises, async behavior, exceptions, or host calls.
- Non-serializable component state and values without a stable Wasm representation.
- Expressions that cannot be evaluated identically by the Rust IR runtime and the AssemblyScript backend.

SDK typings are necessary but not sufficient. The Studio validator enforces the portable subset semantically before either backend accepts a module.

---

## 6. Proposed Studio IR

The first IR should be deliberately small, typed, serializable for diagnostics/testing, and independent of GPUI, `rsvelte`, and AssemblyScript implementation types.

```rust
struct StudioModule {
    id: ModuleId,
    source: SourceFile,
    imports: Vec<Import>,
    exports: Vec<Export>,
    components: Vec<ComponentDefinition>,
    dependencies: Vec<ModuleId>,
}

struct ComponentDefinition {
    id: ComponentId,
    name: String,
    props: Vec<PropDefinition>,
    state: Vec<StateSlot>,
    derived: Vec<DerivedSlot>,
    handlers: Vec<EventHandler>,
    template: Vec<TemplateNode>,
    span: SourceSpan,
}

struct StateSlot {
    id: StateSlotId,
    name: String,
    ty: StudioType,
    initial: Expression,
    span: SourceSpan,
}

enum TemplateNode {
    Component {
        id: NodeId,
        kind: ComponentKind,
        explicit_key: Option<Expression>,
        props: Vec<PropBinding>,
        children: Vec<TemplateNode>,
        span: SourceSpan,
    },
    Text {
        id: NodeId,
        value: String,
        span: SourceSpan,
    },
    Interpolation {
        id: NodeId,
        expression: Expression,
        span: SourceSpan,
    },
    If {
        id: NodeId,
        condition: Expression,
        consequent: Vec<TemplateNode>,
        alternate: Vec<TemplateNode>,
        span: SourceSpan,
    },
    Each {
        id: NodeId,
        collection: Expression,
        item: LocalId,
        index: Option<LocalId>,
        key: Expression,
        body: Vec<TemplateNode>,
        span: SourceSpan,
    },
}

enum Expression {
    Literal(StudioValue),
    ReadProp(PropId),
    ReadState(StateSlotId),
    ReadDerived(DerivedSlotId),
    ReadLocal(LocalId),
    Unary {
        operator: UnaryOperator,
        operand: Box<Expression>,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Conditional {
        condition: Box<Expression>,
        consequent: Box<Expression>,
        alternate: Box<Expression>,
    },
    Array(Vec<Expression>),
    Record(Vec<(FieldId, Expression)>),
    CallApprovedFunction {
        function: FunctionId,
        arguments: Vec<Expression>,
    },
}
```

Additional IR operations will represent state assignment, event emission, approved function bodies, and lifecycle behavior. Raw JavaScript text is not an executable IR node. Unsupported source constructs produce diagnostics instead of being carried through for a later runtime to interpret.

Stable IDs must derive from module identity and structural/source identity rather than allocation order alone. They are used by diagnostics, dependency tracking, state preservation, and hot swapping.

---

## 7. Rust crate shape

```text
crates/
  studio-script/
    src/
      lib.rs                  # stable Studio-facing API
      split.rs                # script-block convention
      source.rs               # source files, spans, fingerprints
      diagnostics.rs
      rsvelte_adapter.rs      # the only module coupled to rsvelte
      validate.rs             # portable-subset and SDK validation
      types.rs                # Studio type system
      ir/
        mod.rs
        expression.rs
        module.rs
        template.rs
      lower/
        assemblyscript.rs     # Studio IR → ASC-compatible source
      runtime/
        evaluate.rs           # expression and handler evaluator
        hot_swap.rs           # atomic IR replacement/state migration
```

The public boundary should remain Studio-owned:

```rust
fn prepare(path: &Path, source: &str, target: Target)
    -> Result<PreparedSource, StudioScriptError>;

fn compile(prepared: PreparedSource)
    -> Result<StudioModule, StudioScriptError>;
```

Callers should not receive `rsvelte` types. Pin the selected `rsvelte` revision, isolate it behind `rsvelte_adapter`, and add compatibility fixtures for every Svelte feature Studio relies upon. Updating `rsvelte` then becomes a controlled adapter/test change rather than a workspace-wide migration.

The published npm/Wasm compiler version and the embeddable Rust facade are not assumed to share identical package availability or versioning. Dependency selection must be verified and locked before integration.

---

## 8. Identity-preserving development hot swap

The Rust development runtime owns the live component implementation. A successful edit produces a replacement `ComponentDefinition`, not executable JavaScript.

```rust
let prepared = studio_script::prepare(&path, &source, Target::Development)?;
let replacement = studio_script::compile(prepared)?;

dev_runtime.replace_module_atomically(replacement, |old, new| {
    preserve_compatible_state(old, new)
})?;
```

The swap contract should be:

1. Parse, validate, and lower the complete changed module before touching live state.
2. Reject invalid IR and keep the last valid component running.
3. Match components by stable module/component identity.
4. Match retained nodes by explicit key first, then stable structural identity.
5. Preserve a state slot only when its stable ID and `StudioType` remain compatible.
6. Initialize new slots and dispose removed derived values, effects, handlers, and nodes.
7. Apply the replacement transactionally and roll back if host validation fails.
8. Invalidate and reevaluate dependent IR modules in dependency order.

This is HMR at the Studio IR boundary. GPUI widgets retain identity where the contract permits, while their props, children, bindings, and handlers are replaced.

---

## 9. Build and resume lifecycle

Studio source is always authoritative. Generated IR, AssemblyScript, and Wasm are cacheable artifacts identified by fingerprints over:

- Source contents and resolved dependency graph.
- Studio Script language/IR version.
- Studio SDK and generated binding versions.
- `rsvelte` adapter/compiler revision.
- ASC version and relevant compiler options.
- Target ABI and host protocol version.

The expected lifecycle is:

```text
studio dev starts or resumes
    ├─ load/parse the latest sources
    ├─ rebuild the development IR cache when stale
    ├─ optionally refresh a stale dev Wasm snapshot once at the session boundary
    └─ use IR hot swaps for subsequent edits without rebuilding Wasm

studio build
    └─ lower the latest valid IR to AssemblyScript and compile Wasm

studio release
    └─ perform a clean validated ASC → Wasm build and package the result
```

Edits during a development session mark the Wasm artifact stale but do not force an ASC build. The latest source is compiled on the next explicit build/release, or once on a later start/resume if that workflow requires an up-to-date Wasm snapshot.

This process is local and deterministic after dependencies and compiler artifacts are installed. Loss of internet access or exhaustion of agent service credits must not block parsing, IR hot swap, ASC compilation, or resuming work. Agents may author source, but they are not part of the build chain.

To prevent drift between the Rust evaluator and ASC output, CI and an optional local check should run representative IR fixtures through both paths and compare observable Studio protocol messages.

---

## 10. Production lowering

Production does not compile `rsvelte`-generated JavaScript with ASC. Instead:

```text
.studio source
    ↓
rsvelte AST
    ↓
validated typed Studio IR
    ↓
AssemblyScript backend
    ├─ component modules
    ├─ static dependency imports
    ├─ Studio SDK calls
    └─ generated application bootstrap
    ↓
ASC
    ↓
module.wasm
```

The AssemblyScript SDK already supplies the host protocol bindings, retained widget representation, events, navigation types, and reactive primitives. The lowerer maps Studio IR operations onto those APIs. The SDK remains the Wasm-side runtime contract; the Studio validator remains responsible for proving that source constructs have supported AssemblyScript semantics.

---

## 11. Module graph and virtual modules

Eliminating QuickJS does not eliminate module management. `studio-dev` still needs a module graph to resolve imports, track dependencies, invalidate changed modules, order dependent reevaluation, and compute deterministic build inputs.

Normal `.studio` components are ordinary source modules and do not require one virtual module per component.

Virtual modules remain reserved for generated or host-provided contracts such as:

```text
@studio/generated/routes
@studio/generated/assets
@studio/dev-runtime
@studio/bootstrap
```

The bootstrap may be materialized only for the ASC build. During development the Rust host can bootstrap directly from the root module's IR.

---

## 12. Responsibilities

| Piece | Owner | Responsibility |
|---|---|---|
| Svelte-compatible parsing and projection | `rsvelte` | Parse source syntax, preserve spans, report source facts |
| Compiler boundary and adapter | Rust `studio-script` | Hide `rsvelte`, normalize AST, own diagnostics and versioning |
| Studio type checking and portable-subset validation | Rust `studio-script` | Guarantee that supported constructs have Rust and ASC semantics |
| Studio IR | Rust `studio-script` | Stable target-neutral representation of component behavior |
| Development execution and HMR | Rust IR runtime plus host | Evaluate IR and swap implementations while preserving identity |
| Production source emission | Studio AssemblyScript lowerer | Generate ASC-compatible modules from typed IR |
| Wasm compilation | ASC | Compile generated AssemblyScript to `module.wasm` |
| Native loading and rendering | Wasmtime plus Studio host/GPUI | Enforce sandbox/ABI policy and draw retained native UI |
| Web loading | Browser host adapter | Load Wasm and translate Studio protocol output to the web renderer |

---

## 13. Suggested implementation order

The immediate deliverable is the compiler/runtime path in steps 1–11. Broader editor and ecosystem integrations are deliberately deferred until the source language, IR, diagnostics, and module graph are stable; otherwise those tools would be built against moving contracts.

1. Keep `studio-script` as the only public compiler boundary and move source splitting into its own module.
2. Pin an embeddable `rsvelte` revision and implement the private adapter with compatibility fixtures.
3. Define source spans, stable IDs, `StudioType`, expressions, templates, modules, and versioned IR fixtures.
4. Lower the example `.studio` component from `rsvelte` AST into Studio IR.
5. Implement portable-subset validation and closed SDK/component catalog resolution.
6. Implement the Rust expression/handler evaluator and retained template interpreter.
7. Implement transactional, identity-preserving IR module replacement.
8. Implement Studio IR → AssemblyScript lowering using the existing SDK.
9. Add fingerprinted caches and start/resume/build/release lifecycle behavior.
10. Add differential fixtures proving equivalent Rust-dev and ASC/Wasm behavior.
11. Connect the Rust file watcher and module graph to the live GPUI host.
12. Add a browser host adapter only when the web target is scheduled.

Each stage should include focused diagnostics, malformed-input tests, deterministic-output tests, dependency invalidation tests, and compatibility tests before the next integration stage begins.

---

## 14. Relation to the roadmap

This pipeline belongs to **Feature 004 — Studio Script and Embedded Development Host** in [ROADMAP.md](ROADMAP.md). It depends on **003 — Studio Toolchain and Development Workflow** for reliable watching, cancellation, content-stable generation, deterministic module graphs, and one active build at a time.

`studio dev` should reuse shared host and rendering libraries rather than launching or duplicating the complete `studio-app` executable:

```text
studio-app  → studio-host + studio-shell + studio-renderer + studio-wasm
studio dev  → studio-dev + studio-script + studio-host + studio-shell + studio-renderer
```

## File conventions

```text
app.studio.ts          root application/module configuration
components/*.studio    reusable Studio components
routes/*.studio        file-based routes and route-local components
```

Studio component tags name Studio catalog components, not browser DOM elements.

---

## 15. Future `rsvelte` developer-tooling integrations

After the core pipeline is working, Studio should use more of the `rsvelte` ecosystem to provide a familiar Svelte-quality authoring experience. These integrations are consumers of the stable Studio language and IR contracts; they are not prerequisites for the first compiler, Rust IR runtime, hot swap, or ASC backend.

### Formatting

Use `@rsvelte/fmt` as the formatting engine and add a Studio adapter that:

- Registers `.studio` as a supported source type.
- Preserves Studio component and directive conventions.
- Produces deterministic output suitable for CI.
- Reads the formatter configuration supported by the selected `rsvelte` version rather than pretending to support Prettier configuration compatibility.
- Adds fixtures for script-less components, nested control flow, Studio events, and keyed iteration.

### Linting

Use `@rsvelte/lint` for general Svelte-language diagnostics, then layer Studio-owned rules over the same source spans and module graph:

- Unknown Studio components, props, events, routes, and assets.
- Deprecated SDK/catalog APIs.
- Non-serializable state and unstable/unkeyed retained collections.
- Browser, Node/Bun, dynamic JavaScript, and unauthorized host API usage.
- Expressions outside the portable Rust-evaluator/AssemblyScript subset.
- HMR hazards such as incompatible state-slot changes or unstable component identity.

`@rsvelte/oxlint-plugin` may become an optional faster path when its `.svelte` support covers Studio's required source shapes. Until then, `@rsvelte/lint` plus Studio's Rust validator remains authoritative.

### Type checking and source projection

Use `@rsvelte/svelte2tsx` and `@rsvelte/svelte-check` to improve ordinary TypeScript diagnostics and editor interoperability. The projection layer should inject generated Studio SDK, component catalog, prop, event, route, and asset types.

These checks complement rather than replace the Studio compiler. TypeScript acceptance does not prove that an expression can be represented in Studio IR or compiled by ASC, so the Rust portable-subset validator remains the final authority.

Projection mappings should connect generated TypeScript diagnostics back to exact `.studio` source spans and coexist with the IR's stable IDs and diagnostics.

### Language server

Build a Studio-owned language server that composes the reusable `rsvelte` capabilities available at the pinned revision. It should add the Studio features that the upstream language server does not yet provide:

- Component, prop, event, rune, route, asset, and approved-SDK completion.
- Hover documentation sourced from the Studio catalog and SDK.
- Go-to-definition across `.studio` modules, routes, assets, and SDK declarations.
- Rename and reference search based on the Studio module graph and projected TypeScript.
- TypeScript, Svelte, lint, Studio IR, portability, and build diagnostics in one ordered stream.
- Formatting and quick fixes with correct `.studio` source mappings.

The language server should reuse `studio-script` for parsing/validation contracts rather than implement a second compiler.

### VS Code extension

Provide a Studio extension, informed by `rsvelte-vscode`, that:

- Registers the `.studio` language identifier and syntax grammar.
- Starts and communicates with `studio-language-server`.
- Exposes format, check, build, dev, and restart-session commands.
- Shows route, asset, component, and HMR status without owning compiler state.
- Degrades to syntax highlighting and local commands if optional language features fail.

The extension is a client of the Studio toolchain and must not become part of the compiler or release-build trust boundary.

### Vite, NAPI, and browser tooling

`@rsvelte/vite-plugin-svelte` and `@rsvelte/vite-plugin-svelte-native` can support an optional browser preview, playground, or compatibility bridge for JavaScript build tools. They are not required by native `studio dev`, which evaluates Studio IR in Rust.

The native NAPI compiler can be useful when a JavaScript-hosted tool needs native `rsvelte` performance. The Rust Studio toolchain should use the pinned embeddable Rust facade directly rather than route compilation through Node, NAPI, or the npm/Wasm compiler.

### Tooling ownership and updates

All upstream packages and compiler revisions must be pinned. Studio-owned wrappers should keep package APIs, configuration differences, and experimental limitations out of project source files. An update is accepted only after compatibility fixtures validate:

- Parsing and exact source spans.
- Formatting stability and idempotence.
- Lint and type diagnostic mappings.
- TypeScript projection fidelity.
- Language-server completion/navigation behavior.
- Unchanged Studio IR and ASC/Wasm semantics.

This deferred work is tracked as **Feature 008 — Studio Script Developer Experience** in [ROADMAP.md](ROADMAP.md).
