# Studio Component Catalog Plan

This document defines the planned Studio component surface. Studio exposes a closed,
Flutter-inspired protocol while implementing components with native GPUI layout primitives and
the vendored `gpui-component` library wherever possible.

## Source policy

| Source | Studio use |
| --- | --- |
| GPUI | Native layout, focus, input, rendering, and window primitives |
| gpui-component | Native controls, overlays, feedback, data display, themes, and desktop widgets |
| Flutter | Declarative naming, composition patterns, layout semantics, and navigation concepts |
| shadcn/ui | Component taxonomy, variants, field composition, and visual design references |
| adabraka-ui | Design and animation reference only; not a runtime dependency |

Studio does not expose arbitrary Rust components to guests. Every exposed node must have a closed
protocol kind, bounded properties, typed events, ownership checks, accessibility semantics, and an
AssemblyScript builder.

## Master catalog

### Layout and composition

| Component | GPUI/gpui-component basis | Flutter/shadcn relationship | Priority |
| --- | --- | --- | --- |
| `Box` / `Container` | GPUI `div` | Flutter `Container` | Existing |
| `Row` | GPUI flex row | Flutter `Row` | Existing |
| `Column` | GPUI flex column | Flutter `Column` | Existing |
| `Stack` | GPUI layered layout | Flutter `Stack` | Existing |
| `Grid` | GPUI layout | Flutter `GridView`, shadcn grid patterns | Existing |
| `Spacer` | GPUI flex spacer | Flutter `Spacer` | Existing |
| `Wrap` | GPUI flow layout | Flutter `Wrap` | Planned |
| `AspectRatio` | GPUI constrained layout | Flutter `AspectRatio`, shadcn component | Planned |
| `Expanded` / `Flexible` | Row/Column flex properties | Flutter flex children | Planned property aliases |
| `Padding` / `Align` / `Center` | Box properties | Flutter composition widgets | Planned property aliases |
| `Divider` / `Separator` | gpui-component separator | Flutter `Divider`, shadcn `Separator` | POS batch |
| `Card` | gpui-component GroupBox/custom card | shadcn Card, Flutter Card | POS batch |

### Typography and display

| Component | GPUI/gpui-component basis | Flutter/shadcn relationship | Priority |
| --- | --- | --- | --- |
| `Text` | GPUI text | Flutter `Text`, shadcn typography | Existing |
| `Icon` | gpui-component icon system | Flutter `Icon`, shadcn icons | Existing |
| `Image` | GPUI image/asset primitives | Flutter `Image` | Existing |
| `Badge` | gpui-component Badge | shadcn Badge, Flutter Badge | POS batch |
| `Tag` | gpui-component Tag | shadcn Badge/Tag patterns | POS batch |
| `Avatar` | gpui-component Avatar | shadcn Avatar, Flutter CircleAvatar | POS batch |
| `Empty` | Studio composition of display primitives | shadcn Empty | POS batch |
| `DescriptionList` | gpui-component DescriptionList | shadcn description patterns | Planned |
| `Kbd` | gpui-component keyboard display | shadcn Kbd | Planned |
| `Skeleton` | gpui-component Skeleton | shadcn Skeleton, Flutter loading placeholder | POS batch |

### Forms and interaction

| Component | GPUI/gpui-component basis | Flutter/shadcn relationship | Priority |
| --- | --- | --- | --- |
| `Button` | gpui-component Button | Flutter Button, shadcn Button | Existing |
| `IconButton` | gpui-component IconButton | Flutter IconButton | Existing |
| `ButtonGroup` | gpui-component ButtonGroup | shadcn Button Group | Planned |
| `Toggle` | gpui-component Toggle | Flutter toggle patterns, shadcn Toggle | Planned |
| `ToggleGroup` | Toggle/ButtonGroup composition | shadcn Toggle Group, Flutter segmented controls | Planned |
| `Checkbox` | gpui-component Checkbox | Flutter Checkbox, shadcn Checkbox | Planned |
| `Radio` | gpui-component Radio | Flutter Radio, shadcn Radio Group | Planned |
| `Switch` | gpui-component Switch | Flutter Switch, shadcn Switch | Planned |
| `Slider` | gpui-component Slider | Flutter Slider | Existing |
| `RangeSlider` | Slider composition | Flutter RangeSlider | Planned |
| `TextInput` | gpui-component Input | Flutter TextField, shadcn Input | Existing |
| `TextArea` | gpui-component Input/state | Flutter multiline field, shadcn Textarea | Planned |
| `SecretInput` | Host-owned native secure input | Flutter obscured field, shadcn input | Existing |
| `Field` | Studio composition | shadcn Field, Flutter FormField | Planned |
| `InputGroup` | Input + prefix/suffix composition | shadcn Input Group | Planned |
| `Select` | gpui-component Select | Flutter Dropdown, shadcn Select | Existing |
| `Combobox` | gpui-component Combobox | Flutter Autocomplete, shadcn Combobox | Planned |
| `NumberInput` | gpui-component NumberInput | Flutter numeric field | Planned |
| `OtpInput` | gpui-component OTP input | Flutter PIN/OTP patterns | Planned, host-owned for secrets |

### Feedback, overlays, and menus

| Component | GPUI/gpui-component basis | Flutter/shadcn relationship | Priority |
| --- | --- | --- | --- |
| `Progress` | gpui-component Progress | Flutter LinearProgressIndicator, shadcn Progress | POS batch |
| `ProgressCircle` | gpui-component circular progress | Flutter CircularProgressIndicator | POS batch |
| `Spinner` | gpui-component Spinner | Flutter progress spinner | POS batch |
| `Tooltip` | gpui-component Tooltip | Flutter Tooltip, shadcn Tooltip | POS batch |
| `Popover` | gpui-component Popover | Flutter popup composition, shadcn Popover | Planned |
| `Dialog` | gpui-component Dialog | Flutter AlertDialog, shadcn Dialog | POS batch |
| `AlertDialog` | gpui-component alert dialog | Flutter AlertDialog, shadcn AlertDialog | POS batch |
| `Sheet` | gpui-component Sheet | Flutter BottomSheet, shadcn Sheet | Planned |
| `Toast` / `Notification` | gpui-component Notification | Flutter SnackBar, shadcn Sonner | POS batch |
| `Banner` / `Alert` | gpui-component Alert/composition | Flutter Banner, shadcn Alert | Planned |
| `ContextMenu` | gpui-component menu | Flutter context menu, shadcn Context Menu | Planned |
| `CommandPalette` | gpui-component command/search primitives | shadcn Command | Planned |
| `MenuBar` / `Menubar` | gpui-component menu | Flutter desktop menus, shadcn Menubar | Future |

### Navigation and application structure

| Component | GPUI/gpui-component basis | Flutter/shadcn relationship | Priority |
| --- | --- | --- | --- |
| `Scaffold` | Studio composition of layout regions | Flutter Scaffold | Planned |
| `AppBar` | GPUI layout + text/buttons | Flutter AppBar | Planned |
| `Sidebar` | gpui-component Sidebar | Flutter NavigationRail/Drawer, shadcn Sidebar | Future |
| `NavigationBar` | Button/Toggle group | Flutter NavigationBar | Future |
| `NavigationRail` | Sidebar composition | Flutter NavigationRail | Future |
| `Drawer` | gpui-component Sheet | Flutter Drawer | Future |
| `Tabs` | gpui-component Tabs | Flutter TabBar, shadcn Tabs | Planned |
| `Breadcrumb` | gpui-component Breadcrumb | shadcn Breadcrumb | Planned |
| `Stepper` | gpui-component Stepper | Flutter Stepper | Future |
| `Pagination` | gpui-component Pagination | shadcn Pagination | Future |

### Lists, data, and scheduling

| Component | GPUI/gpui-component basis | Flutter/shadcn relationship | Priority |
| --- | --- | --- | --- |
| `ListView` | GPUI/gpui-component List | Flutter ListView | Existing |
| `ListTile` | ListItem composition | Flutter ListTile, shadcn Item | Planned |
| `SearchableList` | gpui-component SearchableList | shadcn Command/list patterns | Future |
| `VirtualList` | gpui-component VirtualList | Flutter virtual scrolling | Existing basis |
| `DataTable` | gpui-component DataTable | Flutter DataTable, shadcn Table | Future |
| `Tree` | gpui-component Tree | Desktop-only extension | Future |
| `Calendar` | gpui-component Calendar | Flutter Calendar | Future |
| `DatePicker` | gpui-component DatePicker | Flutter DatePicker, shadcn Date Picker | Future |
| `TimePicker` | GPUI composition | Flutter TimePicker | Future |
| `Stepper` | gpui-component Stepper | Flutter Stepper | Future |

### Deliberately deferred

Charts, docking layouts, code editor/highlighter, rich Markdown/TextView, color picker, drag/drop
frameworks, advanced resizable panels, and full settings frameworks remain deferred until a
vertical product requires them.

## First POS slice

The first implementation should be `Card`.

It is the smallest useful bridge between the existing protocol and native gpui-component styling:

1. Keep the existing `Card` node kind and extend its closed properties for background, border,
   radius, padding, and semantic tone.
2. Map the node to a native GPUI/gpui-component card/group container.
3. Add AssemblyScript `Card` builder and typed property helpers.
4. Replace the POS service and order-summary manual panel styling with `Card` nodes.
5. Add focused mapping, patch, accessibility, and screenshot/manual visual checks.

After `Card`, implement `Badge`, `Separator`, and `Avatar` as the next low-risk batch, followed by
`Progress`, `Spinner`, `Tooltip`, and `Toast`.

## Generated artifact ownership — 002 component platform

Source of truth is `crates/studio-protocol` (`ui.rs`, `properties.rs`, `error.rs`). The following are generated and must not be hand-edited:

- `protocol/schemas/*` — canonical JSON Schema for protocol-v1 (run `cargo run -p studio-protocol --bin generate_schema`)
- `sdk/assemblyscript/assembly/generated/*` — generated AssemblyScript bindings owned by the SDK (run `bun run generate:protocol` or `cargo run -p studio-protocol --bin generate_schema && bun run ./scripts/generate-protocol.ts`)
- `sdk/assemblyscript/assembly/components/*` — typed SDK builders for 002 batches (generated from the same protocol inventory)

Feature `002-component-platform` owns the catalog foundation (`T003–T007`) and the display/interaction batches. See `specs/002-component-platform/spec.md` and `tasks.md` for the red-green order.
