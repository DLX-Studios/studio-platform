# Vice City stream countdown

A native Studio example of Svelte-style `.studio` components, file-based routes,
ancestor layouts, reactive props, and host-owned navigation. The coastal sunset
background is a generated, detailed cityscape using violet, coral, and pink accents. It
covers the full native viewport behind the page. Fixed Pricedown title artwork
and a 168-pixel clock provide the oversized stream presentation.

The countdown starts automatically at `10:00`. A bounded host-clock Timer
updates the page once per second. At zero, the clock requests a 700 ms opacity
exit transition, then the host dispatches completion and the page replaces its
route with `/live`. The digits also request a subtle opacity pulse on each tick.
Native renderer tests verify retained opacity fading, reduced-motion behavior,
large text metrics, and background cover geometry. Full application launch and
platform release acceptance remain separate from source checks.

## Run

From the platform workspace root, using a Studio CLI built with feature 016:

```sh
studio check examples/stream-countdown
studio dev examples/stream-countdown
```

The development host opens a native window and watches loose `.studio` files.
Edit an imported component or layout to exercise source-linked hot reload.
An invalid edit must leave the last valid page available.

## Web build

[Open the hosted countdown](https://vice-city-stream-studio.activematch.chatgpt.site)
(owner-private through Sites). The exact project/version/deployment identity is
recorded in `web-hosting.json`; reuse it for future updates.

The same sources also compile to a static browser app:

```sh
studio build examples/stream-countdown --target web
bun scripts/serve-web.ts examples/stream-countdown/build/web 5180
```

Open `http://127.0.0.1:5180/`. The web target reuses the compiled guest and the
Rust router/timer scheduler, adds a retained DOM adapter, and scales the layout
for mobile screens. See [the web host guide](../../sdk/web/README.md) for build
prerequisites, supported components, browser checks and hosting details.

## Source layout

```text
stream-countdown/
├── manifest.json
├── routes/
│   ├── +layout.studio             # Shared shell and default child outlet
│   ├── +page.studio               # / — countdown state and controls
│   ├── live/
│   │   └── +page.studio           # /live — stream-ready screen
│   └── [...rest]/
│       └── +page.studio           # /* — explicit not-found page
├── components/
│   ├── StreamHeader.studio        # Reusable title/subtitle props
│   └── CountdownClock.studio      # Numeric derived clock display
└── assets/
    ├── vice-city-cityscape.png    # Generated viewport-cover cityscape
    ├── stream-starting.svg        # Fixed Pricedown outline title
    └── stream-live.svg            # Fixed Pricedown outline title
```

There is no root `app.studio` in this example. The page files are the entry
sources, and their ancestor `+layout.studio` files compose the surrounding UI.
The compiler generates its route declaration and source map; those files are
not hand-authored assets and must not be added to the source manifest.

The shared layout uses the Svelte 5 default child outlet:

```svelte
<script lang="ts">
  let { children } = $props();
</script>

<Column id="shell">
  {@render children()}
</Column>
```

`<Slot />` is also supported by this native subset. Each component may have one
default outlet. This is the Studio component catalog, so browser HTML, CSS,
server load functions, and SvelteKit HTTP endpoints are not implied.

## Try the journey

1. The root page starts at `10:00` and advances automatically to `9:59`, `9:58`,
   and onward using the host's monotonic clock.
2. **Start now** sets the countdown to zero, requesting the same exit transition
   and delayed `/live` navigation as a naturally completed timer.
   **Reset 10:00** restores the initial value; resetting during the pending exit
   delay cancels that completion and starts a fresh timer interval.
3. At completion, the page prepares a fresh 600-second value for the next visit
   and replaces `/` with `/live`. **Open countdown** returns to a fresh timer.
4. **Go live** is a manual forward-navigation control. It pushes `/live` while
   retaining a partially elapsed countdown. **Back to countdown** then pops the
   stack and resumes that retained value with a fresh interval.
5. **Explore Leonida** requests an undeclared path and reaches the explicit
   catch-all page. **Go back** returns to the previous page.

The automatic completion uses `replace`, so it does not create a countdown
history entry to pop. Use **Open countdown** after automatic completion. A pop
below the root is rejected by the host.

## Props, components, and route parameters

The countdown state belongs to the root page. It passes the numeric `remaining`
prop into `CountdownClock.studio`; that component derives minutes and seconds.
Default component imports resolve relative to the importing source:

```svelte
<script lang="ts">
  import CountdownClock from '../components/CountdownClock.studio';
  let remaining = $state(600);
</script>

<CountdownClock id="starting-clock" remaining={remaining} />
```

Imported instance node IDs and reactive slots are scoped by the instance ID.
Two explicitly authored instances therefore have independent internal state.
Missing required props, unknown props, incompatible types, cycles, and imports
outside the canonical project root are compile errors.

Route layouts have one retained instance per compiled page. Same-name public
page/layout props must have compatible types and defaults. This version does
not implement callback props or custom event forwarding between components;
bind native control handlers within the component that owns them. Imported components containing state, derived slots, or event handlers inside
keyed `each` blocks are rejected until per-key component ownership is implemented.
Stateless markup components can receive row-local props; explicitly authored
reactive instances outside `each` remain independent.

To add a named parameter page, create `routes/streams/[id]/+page.studio` and
read the host-provided scalar string prop:

```svelte
<script lang="ts">
  let { id } = $props();
</script>

<Text id="stream-id">Stream {id}</Text>
```

A request for `/streams/42` supplies `id = '42'`. Layouts can declare the same
scalar route prop. Parameter values are named scalar props in this version;
a SvelteKit-style `params` object is not implemented. Parameterized visits reuse
the declared page's retained state while updating its parameter props.

Navigation handlers request the existing closed Studio protocol:

```ts
function goLive() {
  emit('navigate', { operation: 'push', route: '/live' });
}

function back() {
  emit('navigate', { operation: 'pop' });
}
```

Supported operations are `push`, `replace`, `pop`, `pop_to`, and `reset`.
The Rust host resolves destinations and owns the bounded history. Component
code does not access browser history or select an arbitrary native screen.

## Host-scheduled Timer

The source-only Timer declaration compiles to invisible retained native targets
and existing handlers. It does not add a native widget, browser timer, per-frame
guest callback, or arbitrary host import:

```svelte
<Timer
  id="stream-timer"
  interval_ms={1000}
  until={remaining === 0}
  completion_delay_ms={700}
  ontick={tick}
  oncomplete={streamStarted}
/>
```

Timers require an explicit stable ID, a static integer interval from 1000 to
60000 ms, a boolean `until` expression, and named handlers. The optional completion
delay is an integer from 0 to 60000 ms. A page may contain at most sixteen timers.
Timers must be in a static retained component tree. The compiler scopes imported
timer IDs and emits a signed timer declaration alongside the route declaration.

The host validates target ownership, advances from elapsed deadlines to avoid
drift, and dispatches at most 32 due events per poll. Leaving the page cancels its
active schedule; returning starts a fresh interval over the retained page state.
Once `until` becomes true, ticks stop and completion is delivered once after the
delay. Resetting the status before that delayed event cancels the pending exit.

## Verification and authored viewport

A successful `studio check` proves source discovery and composition. It does
not prove image decoding, native pointer interaction, or signed Runtime launch.
Development uses interpreted IR; explicit packaging lowers the same composed
source to Wasm. Publisher trust and platform release acceptance remain separate
from opening this development example.

The automated [native renderer tests](../../crates/studio-components/tests/native_rendering.rs)
verify that a 700 ms opacity exit remains visible partway through the fade,
reaches zero without replacing native identity or layout, and resolves immediately
with reduced motion. They also verify authored 168-pixel text changes native glyph
metrics and scales with preview zoom, and that the admitted background covers the
viewport while foreground controls remain clickable. These are native control
and geometry assertions; they do not replace the complete application's signed
launch and per-platform release acceptance.

The demo uses closed native presentation props: `Box.background_asset`,
`Text.font_size`, `Text.color`, opacity, and transitions. Its authored layout targets
the default **1440 × 900 desktop preview**. Header artwork defaults to 900 × 120
logical pixels; `StreamHeader.artworkWidth` can be reduced for a narrower authored
layout, and native preview zoom scales the complete presentation. The clock uses
168 logical pixels. This is a fixed desktop composition, not an automatic mobile
breakpoint layout.

## Title artwork attribution

The two fixed title graphics use outlined **Pricedown Black**, created by Ray
Larabie at [Typodermic Fonts](https://typodermicfonts.com/pricedown/). They contain
finished lettering as vector outlines. No font binary or reusable glyph set is
bundled. See Typodermic's [desktop license for finished static output](https://typodermicfonts.com/desktop/).
The cityscape was generated from the sunset artwork reference for this example,
with the people, car, and logo removed. It is bundled locally and requires no
external image requests.
