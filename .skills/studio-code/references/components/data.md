# Lists, data, dates, and times

These examples distinguish native layout surfaces from complete data widgets. Data loading, selection, filtering, and routing remain app/host concerns.

See [shared contracts](contracts.md) for types, events, state, and validation.

## ListTile

Present one list row with optional trailing content.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** 0–1. **Events/state:** No guest input event is wired for this example.

**Current support:** Native row renders label plus at most one child root.

```svelte
<ListTile id="example-list-tile" label="Ada">
  <Badge id="tile-status" label="Active" />
</ListTile>
```

## SearchableList

Declare a scrollable list surface.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Renders children in a scrollable column; automatic search/filtering is not wired.

```svelte
<SearchableList id="example-searchable-list">
  <Column id="searchable-rows"><Text id="searchable-row" text="Ada" /></Column>
</SearchableList>
```

## VirtualList

Declare a list surface with explicit content.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Current adapter renders children in a scrollable column; it does not implement virtualization.

```svelte
<VirtualList id="example-virtual-list">
  <Column id="virtual-rows"><Text id="virtual-row" text="Ada" /></Column>
</VirtualList>
```

## DataTable

Display declared column headers and authored rows.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Native table-like layout; sorting, row selection, pagination, and cell editing are not automatically wired.

```svelte
<DataTable id="example-data-table" columns={["Name", "Status"]}>
  <Row id="table-row" gap={16}><Text id="table-name" text="Ada" /><Text id="table-status" text="Active" /></Row>
</DataTable>
```

## Tree

Display authored hierarchical content.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Indented native surface; automatic expand/collapse and tree-selection events are not wired.

```svelte
<Tree id="example-tree">
  <Column id="tree-branch"><Text id="tree-root" text="Project" /><Text id="tree-leaf" text="app.studio" /></Column>
</Tree>
```

## DescriptionList

Present labels and values in two columns.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Native two-column grid; alternate authored key/value children.

```svelte
<DescriptionList id="example-description-list">
  <Text id="description-key" text="Status" />
  <Text id="description-value" text="Active" />
</DescriptionList>
```

## Calendar

Show a native date-selection surface.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** LIMITED: shares a host date-picker state with DatePicker; guest value/selection synchronization and change dispatch are not wired.

```svelte
<Calendar id="example-calendar" label="Date" />
```

## DatePicker

Show a native date picker.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** LIMITED: host widget renders, but the shared picker state is not bound to guest value props/events.

```svelte
<DatePicker id="example-date-picker" label="Date" />
```

## TimePicker

Edit canonical local time in native segments.

**Props:** `value`: canonical 24-hour `HH:MM` or `HH:MM:SS`; `label`: nonempty string; `enabled`: boolean; protocol event prop `on_changed`.

**Children:** none. **Events/state:** onchange={handler}; native changed payload carries canonical HH:MM or HH:MM:SS. Invalid time text is not dispatched.

**Current support:** Retained state and canonical value binding are implemented; locale-specific strings, dates, and zones are not accepted.

```svelte
<TimePicker id="example-time-picker" label="Start time" value="09:30" enabled={true} />
```
