# Advanced catalog surfaces

Most components here currently use a generic native surface. They accept catalog data and display it, but do not expose the full similarly named upstream GPUI Kit widget. Each entry states exceptions.

See [shared contracts](contracts.md) for types, events, state, and validation.

## Separator

Display a thin separator.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Dedicated native separator; no thickness/orientation-specific prop exists in its current profile.

```svelte
<Separator id="example-separator" />
```

## Accordion

Declare grouped expandable content.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC native surface: renders declared text/collections/children; specialized behavior is not wired.

```svelte
<Accordion id="example-accordion" title="Sections" open={true}>
  <Text id="accordion-content" text="Example content" />
</Accordion>
```

## Collapsible

Gate visibility of authored children with an open prop.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC native surface: renders declared text/collections/children; specialized behavior is not wired.

```svelte
<Collapsible id="example-collapsible" title="Details" open={true}>
  <Text id="collapsible-content" text="Example content" />
</Collapsible>
```

## HoverCard

Declare supporting hover-card content.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC surface: no hover-trigger behavior is implemented by this tag.

```svelte
<HoverCard id="example-hover-card" title="Account details">
  <Text id="hovercard-content" text="Example content" />
</HoverCard>
```

## KeyboardShortcuts

Present keyboard shortcut labels.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC monospace surface; listing keys does not register keybindings.

```svelte
<KeyboardShortcuts id="example-keyboard-shortcuts" keys={["Ctrl+S", "Ctrl+P"]}>
  <Text id="keyboardshortcuts-content" text="Example content" />
</KeyboardShortcuts>
```

## Kbd

Display a keyboard key label.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC monospace display; no keyboard listener is registered.

```svelte
<Kbd id="example-kbd" label="Ctrl+S" />
```

## ColorPicker

Show a native color-selection widget.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** LIMITED: native widget renders from shared host state; guest value and change dispatch are not bound.

```svelte
<ColorPicker id="example-color-picker" label="Accent" value="#336699" />
```

## Rating

Display a rating using the native widget.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Native rating display rounds/clamps value to 0–5. Guest change dispatch is not wired.

```svelte
<Rating id="example-rating" value={4} />
```

## Resizable

Declare resizable-region content.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC surface: no drag resize behavior is wired.

```svelte
<Resizable id="example-resizable" title="Panels">
  <Text id="resizable-content" text="Example content" />
</Resizable>
```

## Dock

Declare docked content.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC surface: no docking manager is wired.

```svelte
<Dock id="example-dock" title="Workspace panels">
  <Text id="dock-content" text="Example content" />
</Dock>
```

## Chart

Declare a labeled data-series surface.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC surface renders declared series strings; it does not plot numeric charts.

```svelte
<Chart id="example-chart" title="Revenue" series={["January", "February"]}>
  <Text id="chart-content" text="Example content" />
</Chart>
```

## Editor

Declare source editor content.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC surface: no text-editor input, syntax highlighting, or language service is wired.

```svelte
<Editor id="example-editor" label="Source" content="let count = 0;">
  <Text id="editor-content" text="Example content" />
</Editor>
```

## RichText

Declare formatted-content text.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC surface renders text; it does not parse HTML or provide a rich-text editor.

```svelte
<RichText id="example-rich-text" content="Project notes">
  <Text id="richtext-content" text="Example content" />
</RichText>
```

## Carousel

Arrange authored content horizontally.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC horizontal surface; paging/slide controls and autoplay are not wired.

```svelte
<Carousel id="example-carousel" title="Gallery">
  <Text id="carousel-content" text="Example content" />
</Carousel>
```

## DragDrop

Declare drag-and-drop content.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC surface: on_drop is a declared protocol prop, but drag/drop dispatch is not wired.

```svelte
<DragDrop id="example-drag-drop" label="Drop target">
  <Text id="dragdrop-content" text="Example content" />
</DragDrop>
```

## Theme

Declare a themed content region.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC surface; this does not switch the host theme or import CSS tokens.

```svelte
<Theme id="example-theme" label="Theme region">
  <Text id="theme-content" text="Example content" />
</Theme>
```

## AspectRatio

Declare aspect-constrained content.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC surface; there is no numeric ratio prop or enforced aspect-ratio layout.

```svelte
<AspectRatio id="example-aspect-ratio" label="Media region">
  <Text id="aspectratio-content" text="Example content" />
</AspectRatio>
```

## Alert

Display alert-style text.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC native surface: renders declared text/collections/children; specialized behavior is not wired.

```svelte
<Alert id="example-alert" content="Connection lost" variant="warning" />
```

## Attachment

Display attachment metadata.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC surface: no attachment upload/download behavior is wired.

```svelte
<Attachment id="example-attachment" label="report.pdf" />
```

## Bubble

Group chat-bubble content.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC native surface: renders declared text/collections/children; specialized behavior is not wired.

```svelte
<Bubble id="example-bubble" label="Sender">
  <Text id="bubble-content" text="Example content" />
</Bubble>
```

## Command

Display a command label.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC surface: no command execution is wired.

```svelte
<Command id="example-command" label="Open project" />
```

## NativeSelect

Declare a selection-like display surface.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC surface; use Select for the retained interactive selector.

```svelte
<NativeSelect id="example-native-select" label="Environment" options={["Local", "Staging"]} value="Local" />
```

## NavigationMenu

Group menu content.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC surface; child actions own navigation.

```svelte
<NavigationMenu id="example-navigation-menu" label="Navigation">
  <Text id="navigationmenu-content" text="Example content" />
</NavigationMenu>
```

## ScrollArea

Group scrollable content.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC scrollable surface; use ScrollView for its declared axis and native scroll-dispatch contract.

```svelte
<ScrollArea id="example-scroll-area" label="History">
  <Text id="scrollarea-content" text="Example content" />
</ScrollArea>
```

## Item

Group a generic catalog item.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC decorated item; use Box as the general div-like layout wrapper.

```svelte
<Item id="example-item" label="Entry">
  <Text id="item-content" text="Example content" />
</Item>
```

## Message

Display message text.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC native surface: renders declared text/collections/children; specialized behavior is not wired.

```svelte
<Message id="example-message" content="Saved successfully" variant="success" />
```

## MessageScroller

Group a scrollable message history.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC scrollable surface; paging, anchoring, and message virtualization are not wired.

```svelte
<MessageScroller id="example-message-scroller" label="Messages">
  <Text id="messagescroller-content" text="Example content" />
</MessageScroller>
```

## ToggleGroup

Group boolean control content.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC surface; nested toggles own state, with no automatic group selection logic.

```svelte
<ToggleGroup id="example-toggle-group" label="View options">
  <Text id="togglegroup-content" text="Example content" />
</ToggleGroup>
```

## Sonner

Display notification-like message text.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** GENERIC alert-style surface; no toast queue or Sonner library API is exposed.

```svelte
<Sonner id="example-sonner" content="Saved" variant="success" />
```
