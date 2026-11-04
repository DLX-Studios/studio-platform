# Layout and general containers

Start with Box for a div-like wrapper. Use Row/Column for explicit flow and Grid for columns. Sizes are logical pixels; the preview applies zoom.

See [shared contracts](contracts.md) for types, events, state, and validation.

## Box

General-purpose wrapper, closest to a div: group content and control padding, size, growth, and semantic background.

**Props:** `padding`, `width`, `height`: finite number 0–10000; `background`: semantic color; `background_asset`: admitted local asset path painted behind content with cover; `flex`: number 0–10; `shrink`: boolean.

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** The renderer uses a vertical flex wrapper. Nest Row/Column for intentional content flow; Box is not an HTML element. In the current adapter, `shrink={true}` calls `flex_shrink_0`, so it prevents shrinking despite the prop name.

```svelte
<Box id="example-box" padding={16} width={360} background="surface_variant">
  <Column id="box-content" gap={8}>
    <Text id="box-title" text="Account" />
    <Button id="box-save" label="Save" />
  </Column>
</Box>
```

## Column

Arrange children vertically.

**Props:** `gap`: number 0–10000; `alignment`: `start`, `center`, `end`, `stretch`, `space_between`; `flex`: number 0–10.

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<Column id="example-column" gap={8} alignment="stretch">
  <Text id="column-one" text="First" />
  <Text id="column-two" text="Second" />
</Column>
```

## Row

Arrange children horizontally.

**Props:** `gap`: number 0–10000; `alignment`: `start`, `center`, `end`, `stretch`, `space_between`; `flex`: number 0–10.

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<Row id="example-row" gap={8} alignment="center">
  <Text id="row-caption" text="Status" />
  <Badge id="row-state" label="Ready" />
</Row>
```

## Stack

Group children in a positioned container.

**Props:** `alignment`: `start`, `center`, `end`, `stretch`, `space_between`.

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Renderer uses a relative flex container; automatic absolute overlay positioning is not implemented by this tag alone.

```svelte
<Stack id="example-stack" alignment="center">
  <Text id="stack-caption" text="Layer content" />
</Stack>
```

## Grid

Place content in a fixed-column native grid.

**Props:** `columns`: integer 1–64; `gap`: number 0–10000.

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<Grid id="example-grid" columns={2} gap={12}>
  <Text id="grid-one" text="One" />
  <Text id="grid-two" text="Two" />
</Grid>
```

## ScrollView

Scroll one content root along an axis.

**Props:** `axis`: `horizontal` or `vertical`.

**Children:** 0–1. **Events/state:** Native scrolled payload contains delta_x/delta_y. Studio Script has no onscroll handler mapping.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<ScrollView id="example-scroll-view" axis="vertical">
  <Column id="scroll-content" gap={8}>
    <Text id="scroll-first" text="First entry" />
    <Text id="scroll-last" text="Last entry" />
  </Column>
</ScrollView>
```

## ListView

Scroll multiple ordered children.

**Props:** `axis`: `horizontal` or `vertical`; `gap`: number 0–10000.

**Children:** any. **Events/state:** Native scrolled payload contains delta_x/delta_y. Studio Script has no onscroll handler mapping.

**Current support:** Renders its children in a scrollable flex layout; this tag does not promise virtualization.

```svelte
<ListView id="example-list-view" axis="vertical" gap={8}>
  <Text id="list-one" text="One" />
  <Text id="list-two" text="Two" />
</ListView>
```

## Spacer

Reserve explicit space.

**Props:** `size`: number 0–10000.

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<Spacer id="example-spacer" size={16} />
```

## Divider

Draw a separator with explicit thickness.

**Props:** `thickness`: positive number.

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<Divider id="example-divider" thickness={1} />
```
