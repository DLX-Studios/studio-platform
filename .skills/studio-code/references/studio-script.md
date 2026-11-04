# Studio Script authoring

## A checked starting shape

This uses the same supported constructs as the blank template:

```svelte
<script lang="ts">
  let count = $state(0);
  let total = $derived(count * 2999);

  function increment() {
    count += 1;
  }
</script>

<Card id="counter">
  <Column id="counter-content" gap={8}>
    <Text id="counter-title" typography_role="headline" text="Counter" />
    <Text id="counter-value">{count}</Text>
    <Text id="counter-total">Total ${total}</Text>
    <Button id="counter-add" label="Add" onclick={increment} />
  </Column>
</Card>
```

Keep app source in `.studio` text files. A `.studio` release archive under
`build/` has the same extension but is packaged output. Edit source, not that
archive or generated IR/AssemblyScript/Wasm.

## Svelte-like syntax with Studio semantics

| Familiar construct | Supported Studio use |
| --- | --- |
| `<script lang="ts">` | Portable typed source; Studio owns evaluation/lowering. |
| `$props()` | Destructured instance props; literal defaults establish types. |
| `$state(...)` | Reactive slots initialized from supported portable values. |
| `$derived(...)` | Recompute a supported expression from props/state. |
| `{count}`, `disabled={...}` | Text interpolation and typed component prop expressions. |
| `onclick={increment}` | Named handlers with supported mutations; use catalog event contracts. |
| `{#if condition}...{:else}...{/if}` | Conditional template content. |
| `{#each items as item (item.id)}` | Supported keyed collections; preserve stable primitive keys and verify the specific shape. |

The validator currently admits the `props`, `state`, and `derived` runes.
It rejects other runes, including `$effect`. It rejects browser globals,
`fetch`, Node/process APIs, dynamic imports, `async`/`await`, promises, JavaScript timers,
and reflective/eval constructs. It is not a general JavaScript engine.
The expression lowerer has a closed set of operators and approved calls;
recognizing a function in JavaScript or TypeScript is not enough to use it.
Keep initial state types explicit through portable literals and verify more
complex shapes with `studio check` before expanding them through the app.

Use named handlers demonstrated by the installed templates, such as
`onclick={increment}`. The validator maps `onclick` to `pressed`, `onchange`
to `changed`, and `onsubmit` to `submitted`, but each component must support
the corresponding event. Do not assume DOM event objects, arbitrary event
names, browser form behavior, or every Svelte binding directive works.

Keep state slot names, types, and component identity stable where practical
during hot reload. Do not promise every structural edit preserves all state.
Read the applied revision and diagnostics; a failed edit leaves the previous
preview active.

## Native components and host behavior

See the [100-component reference](components.md) for examples, props,
cardinality, events, and current support. [Box](components/layout.md#box) is
the general native wrapper closest to a div; choose Row/Column for explicit
flow. Read the shared [component contracts](components/contracts.md) for
controlled state and input-event limitations.

Tags refer to Studio's closed native component catalog: for example `Card`,
`Column`, `Row`, `Text`, `Button`, `ScrollView`, and `Scaffold`. Use current
component props such as `gap`, `padding`, `label`, and `typography_role` from
templates and catalog contracts. Give authored nodes descriptive, distinct
IDs, and keep them stable when editing related code. Do not assume browser
elements, arbitrary CSS classes, `<style>` styling, Tailwind, or imported web
components provide native layout behavior.

In a platform checkout, component/event/prop definitions live in
`crates/studio-protocol` and renderer validation in `crates/studio-ui`;
template examples live in `templates/`. Use those definitions and compiler
diagnostics rather than inventing component names or prop types.

The host owns networking, secrets, persistence, and privileged actions. Use
the app's existing checked SDK/host patterns and required manifest
capabilities for such features. `emit(...)` is admitted by supported handler
shapes, but emitting an event does not implement a database call or external
integration. Verify the receiving host action and its contract. Keep
credentials out of guest source/state/messages and use supported opaque
authorization references where required.

## Screens, modules, and build limits

For multiple dev screens, create the necessary source directories and declare
the screen graph in root `studio.project.json`:

```json
{
  "schemaVersion": 1,
  "screens": [
    { "id": "home", "source": "app.studio" },
    { "id": "settings", "source": "screens/settings.studio" }
  ]
}
```

Source paths must be normalized, project-relative `.studio` files inside the
project. The dev host compiles declared entries and exposes their IDs to
`preview.presentation.set`. This preview control is not app navigation code.

For file-based app navigation, use `routes/+page.studio`, nested
`routes/live/+page.studio`, and optional ancestor `+layout.studio` files.
`studio check`, `studio dev` and `studio build` compose these pages through
the same project compiler. An explicit `studio.project.json` keeps precedence
for existing declared-screen preview projects; its presentation control is
separate from app navigation.

Local default imports of `.studio` components support typed scalar props and
independent static instance state. Layouts insert `<Slot />` or declare
`let { children } = $props()` and use `{@render children()}`. Stateful imported
components inside keyed collections and arbitrary npm modules remain outside
the supported subset. Route parameters become string props.

Handlers navigate with `emit('navigate', { operation: 'push', route: '/live' })`;
pop, replace, pop_to and reset use the existing checked host commands. The
Rust host owns matching and history, retaining page state on return.

A source-only `<Timer id="clock" interval_ms={1000} until={remaining === 0}
ontick={tick} oncomplete={finish} completion_delay_ms={700} />` uses the host's
monotonic scheduler. It supports bounded intervals and delayed completion,
cancels on page exit, and requires named handlers and a stable explicit ID.
Do not substitute browser timers or `$effect`.

See `docs/development/STUDIO_FILE_ROUTING.md` and
`examples/stream-countdown/README.md` in the platform checkout for the complete
contract and runnable example. Modern Studio Script formatting is not implied
by `studio fmt`: that command uses the legacy canonical printer, so do not run
it blindly on Svelte-shaped sources.

For compiler truth in a platform checkout, inspect
`crates/studio-script/src/validate.rs`, `lower_studio.rs`, `expression.rs`,
`tests/studio_validate.rs`, `tests/fixtures/ir-v2/`, and `templates/blank/`.
New language features need implementation and verification in both dev and
release paths; do not silently weaken the portable subset in app code.
