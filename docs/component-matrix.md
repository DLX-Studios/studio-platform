# Studio Component Renderer Matrix

This matrix tracks the renderer contract per closed protocol kind. `Native mapped` means the
catalog resolves to a Studio-owned native layer. `Semantically rendered` means the GPUI renderer
honors the declared properties for that kind. `Verified` means automated fixtures cover the
renderer/state contract. A mapped kind is not automatically considered rendered.

| Kind | Family | Protocol declared | Native mapped | Semantically rendered | Verified |
| --- | --- | --- | --- | --- | --- |
| Box | Container | yes | yes | yes | yes |
| Column | Container | yes | yes | yes | yes |
| Row | Container | yes | yes | yes | yes |
| Stack | Container | yes | yes | yes | yes |
| Grid | Container | yes | yes | yes | yes |
| ScrollView | Container | yes | yes | yes | yes |
| ListView | Container | yes | yes | yes | yes |
| Spacer | Container | yes | yes | yes | yes |
| Divider | Container | yes | yes | yes | yes |
| Text | Text | yes | yes | yes | yes |
| Icon | Media | yes | yes | yes | yes |
| Image | Media | yes | yes | yes | yes |
| Card | Container | yes | yes | yes | yes |
| Badge | Text/display | yes | yes | yes | yes |
| Tag | Text/display | yes | yes | yes | yes |
| Avatar | Media | yes | yes | yes | yes |
| Empty | Display | yes | yes | yes | yes |
| Skeleton | Display | yes | yes | yes | yes |
| ProgressIndicator | Feedback | yes | yes | yes | yes |
| ProgressCircle | Feedback | yes | yes | yes | yes |
| Spinner | Feedback | yes | yes | yes | yes |
| Button | Form/input | yes | yes | yes | yes |
| IconButton | Form/input | yes | yes | yes | yes |
| Checkbox | Form/input | yes | yes | yes | yes |
| Radio | Form/input | yes | yes | yes | yes |
| Switch | Form/input | yes | yes | yes | yes |
| Toggle | Form/input | yes | yes | yes | yes |
| ButtonGroup | Form/input | yes | yes | yes | yes |
| Slider | Form/input | yes | yes | yes | yes |
| RangeSlider | Form/input | yes | yes | yes | yes |
| Select | Form/input | yes | yes | yes | yes |
| Combobox | Form/input | yes | yes | yes | yes |
| NumberInput | Form/input | yes | yes | yes | yes |
| TextInput | Form/input | yes | yes | yes | yes |
| TextArea | Form/input | yes | yes | yes | yes |
| Field | Form/input | yes | yes | yes | yes |
| InputGroup | Form/input | yes | yes | yes | yes |
| OtpInput | Form/input | yes | yes | yes | yes |
| SecretInput | Form/input | yes | yes | yes | yes |
| Dialog | Overlay | yes | yes | yes | yes |
| AlertDialog | Overlay | yes | yes | yes | yes |
| Popover | Overlay | yes | yes | yes | yes |
| Sheet | Overlay | yes | yes | yes | yes |
| BottomSheet | Overlay | yes | yes | yes | yes |
| Toast | Feedback | yes | yes | yes | yes |
| Notification | Feedback | yes | yes | yes | yes |
| Banner | Feedback | yes | yes | yes | yes |
| ContextMenu | Overlay | yes | yes | yes | yes |
| CommandPalette | Overlay | yes | yes | yes | yes |
| Tooltip | Overlay | yes | yes | yes | yes |
| Scaffold | Navigation | yes | yes | yes | yes |
| AppBar | Navigation | yes | yes | yes | yes |
| Sidebar | Navigation | yes | yes | yes | yes |
| NavigationBar | Navigation | yes | yes | yes | yes |
| NavigationRail | Navigation | yes | yes | yes | yes |
| Drawer | Overlay/navigation | yes | yes | yes | yes |
| Tabs | Navigation | yes | yes | yes | yes |
| Breadcrumb | Navigation | yes | yes | yes | yes |
| Stepper | Navigation | yes | yes | yes | yes |
| Pagination | Navigation | yes | yes | yes | yes |
| ListTile | Data display | yes | yes | yes | yes |
| SearchableList | Data display | yes | yes | yes | yes |
| VirtualList | Data display | yes | yes | yes | yes |
| DataTable | Data display | yes | yes | yes | yes |
| Tree | Data display | yes | yes | yes | yes |
| DescriptionList | Data display | yes | yes | yes | yes |
| Calendar | Data display | yes | yes | yes | yes |
| DatePicker | Data display | yes | yes | yes | yes |
| TimePicker | Data display | yes | yes | no | no |
| Separator | Container | yes | yes | yes | yes |
| Accordion | Deferred display | yes | yes | yes | yes |
| Collapsible | Deferred display | yes | yes | yes | yes |
| HoverCard | Deferred overlay | yes | yes | yes | yes |
| MenuBar | Navigation | yes | yes | yes | yes |
| StatusBar | Navigation | yes | yes | yes | yes |
| KeyboardShortcuts | Deferred display | yes | yes | yes | yes |
| Kbd | Deferred display | yes | yes | yes | yes |
| ColorPicker | Deferred input | yes | yes | yes | yes |
| Rating | Deferred input/display | yes | yes | yes | yes |
| Resizable | Deferred display | yes | yes | yes | yes |
| Dock | Deferred display | yes | yes | yes | yes |
| Chart | Deferred display | yes | yes | yes | yes |
| Editor | Deferred input/display | yes | yes | yes | yes |
| RichText | Deferred display | yes | yes | yes | yes |
| Carousel | Deferred display | yes | yes | yes | yes |
| DragDrop | Deferred input/display | yes | yes | yes | yes |
| Theme | Deferred display | yes | yes | yes | yes |
| AspectRatio | Container | yes | yes | yes | yes |
| Alert | Deferred feedback | yes | yes | yes | yes |
| Attachment | Deferred data display | yes | yes | yes | yes |
| Bubble | Deferred data display | yes | yes | yes | yes |
| Command | Deferred input | yes | yes | yes | yes |
| NativeSelect | Deferred input | yes | yes | yes | yes |
| NavigationMenu | Navigation | yes | yes | yes | yes |
| ScrollArea | Container | yes | yes | yes | yes |
| Item | Deferred data display | yes | yes | yes | yes |
| Message | Deferred data display | yes | yes | yes | yes |
| MessageScroller | Deferred data display | yes | yes | yes | yes |
| ToggleGroup | Form/input | yes | yes | yes | yes |
| Sonner | Deferred feedback | yes | yes | yes | yes |

## Notes

- `TimePicker` is protocol-declared and native-mapped but has no native time widget mapping yet;
  production rendering hides it and development rendering emits an explicit fallback diagnostic.
- The remaining formerly deferred kinds use a schema-driven native GPUI surface. Their labels,
  content, collection properties, enabled/interactive flags, variants, and children are rendered;
  they are no longer development fallbacks.
- Release certification is derived from the readiness table and fails when any approved kind is
  not semantically rendered and verified. Development fallback diagnostics never satisfy it.
- Data-display kinds express empty and populated states only. Loading/error states are not
  representable under the closed protocol schema; no renderer-side state semantics were invented.
- Overlay kinds (Dialog/AlertDialog/Popover/Sheet/BottomSheet/Drawer/Toast/Notification/
  ContextMenu/CommandPalette) are gated by host-owned dismissal state that resets whenever the
  protocol reports them closed.
