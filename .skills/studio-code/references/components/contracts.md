# Shared component contracts

## General containers and the div analogy

`Box` is the general-purpose native wrapper, closest to a `div`. Use it for
grouping, padding, width/height, flex growth, and semantic backgrounds. Its
renderer currently lays out children vertically. Use `Column` for explicit
vertical flow with gaps/alignment, `Row` for horizontal flow, and `Grid` for
fixed columns. Use `Card` when the host's card treatment is wanted;
`Scaffold` is the full app shell. `Item` is a decorated catalog surface rather
than a replacement for the general layout wrapper.

There is no HTML DOM, CSS cascade, arbitrary `class`/`style` support, or
upstream Rust builder API in `.studio` markup. Do not invent `<Div>`, `<div>`,
or CSS utility props. Put a `Row`/`Column` inside wrappers where the content
flow matters. The [Box example](layout.md#box) shows a complete pattern.

## Values, identity, and child roots

- Write strings as `label="Save"`, numbers as `gap={8}`, booleans as
  `enabled={true}`, and string lists as `options={["One", "Two"]}`. A quoted
  `"8"` is not a numeric property value.
- Nonempty string lists contain 1–256 unique nonempty strings. Image/Avatar
  asset paths start with `assets/`, use `/`, and contain no empty, `.` or `..`
  segments. Referenced assets must actually be admitted by the host/package.
- Semantic backgrounds are `surface`, `surface_variant`, `primary`,
  `secondary`, `error`, `success`, `warning`, or `transparent`; these are host
  tokens. Text additionally supports a closed six-digit `color="#RRGGBB"` prop; arbitrary CSS remains unsupported. Only props declared for that kind apply:
  e.g. `padding` belongs to Box/Card, not every layout component.
- Each example gives an explicit distinct `id`. Preserve stable identities
  and state slot names when modifying an existing app. The compiler also
  produces internal IR identities; do not assume arbitrary structural edits
  retain every widget or selection.
- `none` means a protocol leaf. Text runs on `Text`, `Button`, `Badge`, and
  `Tag` have special compiler content-folding rules; other leaves should use
  their props. Use either explicit text/label or folded child text, not both.
- `0–1` means wrap multiple children in one `Column`, `Row`, or `Box`.
  `exactly 1` means a root is required even when empty. `any` permits multiple
  children. Conditional/each blocks still need to produce a valid mounted
  shape after evaluation.

At the protocol boundary, common metadata includes `visible` (boolean),
`opacity` (0–1), `accessibility_label` (nonempty string), and
`transition={{curve: "linear", duration_ms: 100}}` with a duration ≤60000.
Curves are `linear`, `ease_in`, `ease_out`, or `ease_in_out`. Protocol
admission does not prove every renderer honors a prop. Also verify the
installed source checker's common-property support: the current
`is_known_property` path probes kind-specific props. Do not bypass a source
diagnostic because a property exists at the protocol boundary.

## Navigation and data profile

The following shared protocol profile applies to Scaffold, AppBar, Sidebar,
NavigationBar, NavigationRail, Drawer, Tabs, Breadcrumb, Stepper, Pagination,
ListTile, SearchableList, VirtualList, DataTable, Tree, DescriptionList,
Calendar, and DatePicker:

| Props | Value |
| --- | --- |
| `label`, `title`, `value`, `route` | Nonempty string |
| `selected`, `open`, `enabled` | Boolean |
| `items`, `options`, `columns` | Nonempty unique string list |
| `page`, `pages`, `step` | Unsigned integer |
| `on_changed`, `on_select`, `on_navigate` | Protocol event name |

This is a shared accepted schema, not a promise that every node consumes
every property. Read the per-component support note: many navigation/data
nodes currently provide layout/display only. Metadata alone does not wire
routing, filtering, pagination, or selection. `Drawer` has a different child
contract from other navigation containers.

## Extended profile

The advanced family (plus MenuBar and StatusBar) shares this protocol profile:

| Props | Value |
| --- | --- |
| `label`, `title`, `content`, `variant` | String |
| `value` | String or finite number |
| `open`, `enabled`, `interactive` | Boolean |
| `items`, `options`, `keys`, `series` | Nonempty unique string list |
| `min`, `max`, `step` | Finite number |
| `on_changed`, `on_select`, `on_drop` | Protocol event name |

Most of these kinds use `deferred_native_element`: it renders declared text,
string collections, numeric bounds, and children with basic native styling.
That surface is not a full chart, editor, docking system, or drag/drop engine.
The generic renderer uses `open` for child visibility, `enabled` for disabled
appearance, and `interactive` for focusability; it does not thereby gain
event dispatch. `Rating`, `ColorPicker`, `Separator`, MenuBar, and StatusBar
have dedicated paths; see their support notes instead.

## Events and controlled state

Studio Script supports `onclick={namedHandler}`, `onchange={namedHandler}`,
and `onsubmit={namedHandler}` as trigger families `pressed`, `changed`, and
`submitted`. A syntactically supported handler still requires the component
and native host to dispatch that event. No component should be assumed to
submit a form merely because `onsubmit` compiles.

Names such as `on_pressed`, `on_changed`, `on_ready`, `on_select`,
`on_navigate`, and `on_drop` in the prop tables are **protocol string
metadata**, not aliases for executable Studio Script handlers. Use
`onclick`/`onchange` where mapped. There is no `onready`, `onselect`,
`onnavigate`, `ondrop`, `onclose`, or `onscroll` syntax mapping, and the
portable subset rejects `on:click` and `bind:value`.

Prefer zero-argument named handlers that mutate known `$state` slots. The
current lowerer does not establish arbitrary JavaScript event-parameter or
DOM event-object bindings. Native input payloads may exist without a way to
read them as `event.target.value` or `event.value` in a Studio Script handler.
Examples of static input values therefore do not claim full guest two-way
editing. For native text editing that must persist exact entered values,
verify the supported app/host integration before designing around it.

### Reactive Button

```svelte
<script lang="ts">
  let count = $state(0);
  function increment() {
    count += 1;
  }
</script>
<Column id="reactive-button-content" gap={8}>
  <Text id="reactive-button-count">{count}</Text>
  <Button id="reactive-button" label="Increment" onclick={increment} />
</Column>
```

### Controlled Checkbox

```svelte
<script lang="ts">
  let checked = $state(false);
  function toggle() {
    checked = !checked;
  }
</script>
<Checkbox id="reactive-checkbox" label="Show archived" value={checked} onchange={toggle} />
```

This handles a boolean toggle through known guest state; it does not read a
DOM event. Radio/Switch/Toggle use the same declared boolean shape, with
app-owned grouping for mutually exclusive radios. Boolean native payloads
currently carry strings rather than JSON booleans. Numeric controls carry a
number, non-secret text/select controls carry strings, and RangeSlider's
dispatch currently carries one number rather than a pair.

`SecretInput` differs: raw input remains host-owned. Its readiness event
contains an opaque authorization reference, never the entered secret. A
string prop naming an event cannot fabricate that reference.

## Checking and developing an example

Each fenced Svelte example in a family page is complete `.studio` source.
Write it to a source file in an existing app, check it with `studio check`,
and preview with `studio dev <folder>` or the MCP source/edit/preview tools.
For overlays, use one example at a time; opening several modals at once is
not a useful smoke test. Runtime requires the app manifest and any assets.

Compiler checks cover supported syntax, names, and known static child
shapes. Property **values**, cross-property rules, and evaluated child
counts still undergo protocol validation when mounted. A passing source
check is not evidence of event delivery, asset loading, rendering, keyboard
behavior, or complete upstream widget functionality. Read `preview.status`
and `preview.logs` after mounting and after saved source edits.
