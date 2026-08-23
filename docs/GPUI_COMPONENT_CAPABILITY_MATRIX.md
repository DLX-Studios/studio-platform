# GPUI Component Capability Matrix

This inventory is generated from the vendored `gpui-component` source and is the source of truth
for runtime adapter work. Components are grouped by the amount of host state they require.

## Direct, stateless elements

These can be rendered from protocol properties and children without a retained entity:

`Badge`, `Tag`, `Separator`, `Skeleton`, `Spinner`, `Progress`, `ProgressCircle`, `Rating`,
`Kbd`, `Label`, `Link`, `GroupBox`, `DescriptionList`, `Alert`, `Accordion`, `Collapsible`,
`HoverCard`, `StatusBar`, `Breadcrumb`, `Stepper`, `Pagination`, `Avatar`, `ButtonGroup`,
`Toggle`, `Checkbox`, `Radio`, `Switch`, `Button`, `IconButton`, `DropdownButton`.

## Stateful controls

These require a host-owned GPUI entity keyed by Studio node ID:

| Component | Upstream state | Primary events |
|---|---|---|
| Input/Textarea | `InputState` | text changed, submit, focus |
| NumberInput | `InputState` | value changed |
| OtpInput | `InputState` | value changed, submit |
| Slider | `SliderState` | value changed |
| Select/Combobox | searchable-list/select state | selection, open changed |
| ColorPicker | `ColorPickerState` | color changed, open changed |
| DatePicker | `DatePickerState` | date selected, open changed |
| Calendar | `CalendarState` | date selected, range changed |
| Popover | `PopoverState` | open changed, dismissed |
| DataTable/Table | `TableState<D>` | row select, sort, resize |
| SearchableList | searchable-list state | selection, query changed |
| Tree | `TreeState` | select, expand/collapse |
| VirtualList | scroll handle/delegate | visible range, scroll |
| TextView/Editor | `TextViewState` or input state | text/selection changed |
| Dock/Resizable | dock/resizable state | panel resize, activate, close |

## Host lifecycle or overlay components

These require focus, dismissal, or host lifecycle coordination in addition to rendering:

`Dialog`, `AlertDialog`, `Sheet`, `Popover`, `Tooltip`, `ContextMenu`, `Notification`,
`FocusTrap`, `Menu`, `MenuBar`, `Sidebar`, `Tabs`, `Drawer`.

## Runtime policy

1. Every stateful component gets a retained state entry keyed by `(plugin_instance, node_id)`.
2. Protocol properties are converted into upstream state only during mount or explicit controlled
   updates; user edits are not overwritten by unrelated patches.
3. Upstream events are normalized to the shared Studio event vocabulary before guest delivery.
4. Unmount, replacement, plugin termination, and compositor loss dispose state and subscriptions.
5. Components absent from this matrix remain Studio compositions built from GPUI primitives.

