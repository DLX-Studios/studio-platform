# Studio file routing

Studio compiles Svelte-style .studio pages into native retained UI. File discovery, layout/component composition and page declarations are shared by `studio check`, `studio dev` and `studio build`. Rust `studio-navigation` owns route resolution/history; no browser runtime is involved.

## Project shape

```text
manifest.json
routes/
  +layout.studio
  +page.studio
  live/+page.studio
  streams/[id]/+page.studio
  [...rest]/+page.studio
components/
  CountdownClock.studio
assets/
```

`+page.studio` maps its directory to a page; root becomes `/`. Named/index .studio files remain shorthand. `+layout.studio` wraps descendants in ancestor order and never creates a page. Private underscore-prefixed files/directories are excluded. Duplicate/ambiguous/malformed/escaping routes fail with source-linked diagnostics. Legacy .ts projects retain their build pipeline; use .studio consistently within an automatically discovered routed project. An explicit `studio.project.json` remains authoritative over automatic discovery for existing declared-screen dev projects.

## Components and layouts

Use local default imports, such as `import Clock from '../components/CountdownClock.studio'`. Declare props with `$props()`; required/default values and known primitive types are checked. Each static component instance gets its own state/derived/handler/node identity. Unknown props, cycles, missing imports, source-root escapes and generated identity collisions fail before commit. Components and their imports are watched; errors retain the previous live revision.

A layout declares `let { children } = $props()` and inserts `{@render children()}`; `<Slot />` is also supported. One default outlet is supported, including ordinary imported wrapper components receiving children. Layout state is scoped per compiled page. Scalar page/layout props with the same name must agree in type/default; named route params are supplied as string props, for example `let { id } = $props()` in a `[id]` page. Param changes reuse that page's state. Composite page stores, server loaders, browser modules and arbitrary npm execution are outside this native subset.

Stateful/derived/event-bearing imported components inside keyed Each blocks are rejected until per-key component state ownership exists; stateless row-local markup is supported. Custom callback/event forwarding across imported component interfaces is not supported; native event handlers live inside their owning component.

## Host navigation

```svelte
<script lang="ts">
  function openLive() {
    emit('navigate', { operation: 'push', route: '/live' });
  }
  function back() {
    emit('navigate', { operation: 'pop' });
  }
</script>
<Button id="open-live" onclick={openLive}>Go live</Button>
<Button id="back" onclick={back}>Back</Button>
```

Operations are the existing push, replace, pop, pop_to and reset commands. The host validates declared destinations and its owner-bound stack (depth 32), then accepts exactly the selected page's mount. Rejected navigation leaves the committed page/history intact. Route-local page state survives forward/back navigation and compatible hot reloads. The compiler packages closed versioned route metadata as a signed reserved asset; authored assets cannot overwrite routing metadata.

Overlapping patterns now compare the first differing segment: static before parameter before wildcard; exact endings beat an empty wildcard sibling. Existing non-overlapping routes/global `/*` retain their behavior. Nested catch-alls match only their prefix. Concrete navigation rejects unsafe paths; CLI/SDK matching retains documented query/trailing-slash normalization.

## Host timers

```svelte
<script lang="ts">
  let remaining = $state(600);
  function tick() { remaining = remaining > 0 ? remaining - 1 : 0; }
  function live() { remaining = 600; emit('navigate', { operation: 'replace', route: '/live' }); }
</script>
<Timer id="stream-timer" interval_ms={1000} until={remaining === 0}
  completion_delay_ms={700} ontick={tick} oncomplete={live} />
```

Timer is a compiler-only declaration lowered into invisible native status/event nodes. The host validates signed metadata and dispatches existing pressed events on its monotonic clock. No browser timers, native kind, Wasm import or per-frame guest callback is added. Intervals are 1000–60000 ms; completion delay is 0–60000 ms. There are at most 16 timers per active page and 32 events per host poll. Catch-up retains elapsed deadlines. The host rechecks status between callbacks, stops ticks on completion and sends completion once after the optional exit delay. Resetting to running before completion dispatch cancels that pending exit. Leaving a page cancels its schedule; return resumes retained state with a fresh interval.

## Presentation

Box.background_asset references an admitted local image painted behind content with viewport cover when the Box fills the screen. Text.font_size is 8–256 logical pixels and respects preview zoom; Text.color is exactly #RRGGBB. Existing opacity/transition props interpolate retained native values and respect reduced motion. No CSS or working font files are needed for the example's fixed outlined Pricedown header artwork.

## Run the example

```sh
studio check examples/stream-countdown
studio dev examples/stream-countdown
```

The countdown begins at 10:00, pulses its digits, fades for 700 ms at zero, then replaces the page with `/live`. The example also demonstrates imports, a shared layout, manual push/back and a not-found page. An explicit package build uses the same composed pages; signing/trust and supported-platform release acceptance remain separate from a source check. See `specs/016-studio-file-routing/validation-report.md` for executed evidence.
