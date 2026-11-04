# Content and feedback

These nodes display content. Asset-backed examples need actual admitted files. Read contracts.md before combining text props and child text.

See [shared contracts](contracts.md) for types, events, state, and validation.

## Text

Display static or reactive text.

**Props:** `text`: string; `typography_role`: `body`, `label`, `caption`, `title`, `headline`, `display`; `font_size`: finite number 8–256 logical pixels; `color`: exactly `#RRGGBB`. Preview zoom scales authored text size. Use text content or the `text` prop, not both.

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Dedicated native display rendering.

```svelte
<Text id="example-text" typography_role="body" text="Hello Studio" />
```

## Icon

Display a named icon.

**Props:** `name`: nonempty icon name.

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Icon name availability depends on the host icon set.

```svelte
<Icon id="example-icon" name="search" />
```

## Image

Display an admitted packaged image asset.

**Props:** `asset`: project asset path; `alt`: nonempty string; `width`, `height`: number 0–10000.

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Requires the matching real asset and manifest admission; compiler checking does not load an image.

```svelte
<Image id="example-image" asset="assets/photo.png" alt="Example photo" width={160} height={100} />
```

## Card

Group a single content root with the host card treatment.

**Props:** `padding`: number 0–10000.

**Children:** 0–1. **Events/state:** No guest input event is wired for this example.

**Current support:** Use Box for an undecorated generic wrapper; Card supplies host card styling.

```svelte
<Card id="example-card" padding={16}>
  <Column id="card-content" gap={8}>
    <Text id="card-title" text="Summary" />
    <Text id="card-detail" text="Account details" />
  </Column>
</Card>
```

## Badge

Display a compact label.

**Props:** `label`: nonempty string.

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Dedicated native display rendering.

```svelte
<Badge id="example-badge" label="3 new" />
```

## Tag

Display a semantic status label.

**Props:** `label`: nonempty string; `variant`: `default`, `success`, `warning`, `destructive`.

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Dedicated native display rendering.

```svelte
<Tag id="example-tag" label="Saved" variant="success" />
```

## Avatar

Represent a person with an asset or fallback text.

**Props:** `asset`: project asset path; `alt`, `fallback`: nonempty strings.

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Asset images require a real admitted asset; this example uses the fallback.

```svelte
<Avatar id="example-avatar" fallback="SW" alt="Example account" />
```

## Empty

Explain why content is absent.

**Props:** `title`, `description`: nonempty strings.

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Dedicated native display rendering.

```svelte
<Empty id="example-empty" title="No entries" description="Add an entry to begin" />
```

## Skeleton

Show a loading placeholder rectangle.

**Props:** `width`, `height`: number 0–10000.

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Dedicated native display rendering.

```svelte
<Skeleton id="example-skeleton" width={160} height={24} />
```

## ProgressIndicator

Display linear progress.

**Props:** `value`: number 0–1 (a fraction, not a percentage).

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Dedicated native display rendering.

```svelte
<ProgressIndicator id="example-progress-indicator" value={0.6} />
```

## ProgressCircle

Display circular progress.

**Props:** `value`: number 0–1 (a fraction, not a percentage).

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Dedicated native display rendering.

```svelte
<ProgressCircle id="example-progress-circle" value={0.6} />
```

## Spinner

Display indeterminate work.

**Props:** `label`: nonempty string.

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Dedicated native display rendering.

```svelte
<Spinner id="example-spinner" label="Loading" />
```
