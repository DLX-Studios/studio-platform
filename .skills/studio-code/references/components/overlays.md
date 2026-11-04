# Overlays and notifications

Visibility and dismissal differ by adapter. Use an app-owned open state where declared; a visible confirmation panel does not authorize an external action.

See [shared contracts](contracts.md) for types, events, state, and validation.

## Dialog

Show a modal content panel controlled by app state.

**Props:** `title`: nonempty string; `open`: boolean.

**Children:** 0–1. **Events/state:** No guest input event is wired for this example.

**Current support:** open controls visibility. Provide an app-controlled way to close it; no onclose handler syntax is mapped.

```svelte
<Dialog id="example-dialog" title="Details" open={true}>
  <Column id="dialog-content"><Text id="dialog-copy" text="Details go here" /></Column>
</Dialog>
```

## AlertDialog

Show a modal confirmation message.

**Props:** `title`, `message`: nonempty strings; `open`: boolean.

**Children:** 0–1. **Events/state:** No guest input event is wired for this example.

**Current support:** A dialog does not perform a host action or grant authorization by itself; wire the action explicitly.

```svelte
<AlertDialog id="example-alert-dialog" title="Remove entry" message="Confirm before proceeding" open={true}>
  <Button id="confirm-remove" label="Confirm" />
</AlertDialog>
```

## Popover

Display a floating content surface.

**Props:** `open`: boolean.

**Children:** 0–1. **Events/state:** No guest input event is wired for this example.

**Current support:** Guest open state controls visibility; the native overlay/popup adapter owns placement and focus. No general onclose syntax is mapped.

```svelte
<Popover id="example-popover" open={true}>
  <Column id="popover-content"><Text id="popover-copy" text="Panel content" /></Column>
</Popover>
```

## Sheet

Display a side panel.

**Props:** `open`: boolean.

**Children:** 0–1. **Events/state:** No guest input event is wired for this example.

**Current support:** Guest open state controls visibility; the native overlay/popup adapter owns placement and focus. No general onclose syntax is mapped.

```svelte
<Sheet id="example-sheet" open={true}>
  <Column id="sheet-content"><Text id="sheet-copy" text="Panel content" /></Column>
</Sheet>
```

## BottomSheet

Display a bottom panel.

**Props:** `open`: boolean.

**Children:** 0–1. **Events/state:** No guest input event is wired for this example.

**Current support:** Guest open state controls visibility; the native overlay/popup adapter owns placement and focus. No general onclose syntax is mapped.

```svelte
<BottomSheet id="example-bottom-sheet" open={true}>
  <Column id="bottomsheet-content"><Text id="bottomsheet-copy" text="Panel content" /></Column>
</BottomSheet>
```

## Drawer

Display a navigation/content drawer.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** 0–1. **Events/state:** No guest input event is wired for this example.

**Current support:** Guest open state controls visibility; the native overlay/popup adapter owns placement and focus. No general onclose syntax is mapped.

```svelte
<Drawer id="example-drawer" open={true}>
  <Column id="drawer-content"><Text id="drawer-copy" text="Panel content" /></Column>
</Drawer>
```

## Toast

Display a transient-style message.

**Props:** `message`: nonempty string.

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Native message surface is implemented; no guest-facing duration, action list, or dismissal callback prop is declared.

```svelte
<Toast id="example-toast" message="Changes saved" />
```

## Notification

Display a notification message.

**Props:** `message`: nonempty string.

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Native message surface is implemented; no guest-facing duration, action list, or dismissal callback prop is declared.

```svelte
<Notification id="example-notification" message="Changes saved" />
```

## Banner

Display an inline message banner.

**Props:** `message`: nonempty string.

**Children:** none. **Events/state:** No guest input event is wired for this example.

**Current support:** Native message surface is implemented; no guest-facing duration, action list, or dismissal callback prop is declared.

```svelte
<Banner id="example-banner" message="Changes saved" />
```

## ContextMenu

Declare a context menu content root.

**Props:** `message`: nonempty string.

**Children:** exactly 1. **Events/state:** No guest input event is wired for this example.

**Current support:** LIMITED: renderer checks open, but the protocol accepts only message (plus common props). This example is declared/compiled but normally remains hidden; no working open trigger is claimed.

```svelte
<ContextMenu id="example-context-menu" message="Entry actions">
  <Column id="context-actions"><Button id="context-open" label="Open" /></Column>
</ContextMenu>
```

## CommandPalette

Declare a command surface with one content root.

**Props:** `placeholder`: nonempty string; `open`: boolean; `commands`: nonempty unique string list.

**Children:** exactly 1. **Events/state:** No guest input event is wired for this example.

**Current support:** Native panel renders command labels; filtering, keyboard command execution, and command-selection dispatch are not provided by this example.

```svelte
<CommandPalette id="example-command-palette" placeholder="Choose command" commands={["Open", "Save"]} open={true}>
  <Text id="palette-hint" text="Choose an available action" />
</CommandPalette>
```

## Tooltip

Describe one wrapped target on hover.

**Props:** `message`: nonempty string.

**Children:** exactly 1. **Events/state:** No guest input event is wired for this example.

**Current support:** Use exactly one target root; put onclick on the nested action control.

```svelte
<Tooltip id="example-tooltip" message="Save changes">
  <Button id="tooltip-target" label="Save" />
</Tooltip>
```
