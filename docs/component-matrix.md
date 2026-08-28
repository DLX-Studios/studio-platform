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
| Accordion | Deferred display | yes | yes | no | no |
| Collapsible | Deferred display | yes | yes | no | no |
| HoverCard | Deferred overlay | yes | yes | no | no |
| MenuBar | Navigation | yes | yes | no | no |
| StatusBar | Navigation | yes | yes | no | no |
| KeyboardShortcuts | Deferred display | yes | yes | no | no |
| Kbd | Deferred display | yes | yes | no | no |
| ColorPicker | Deferred input | yes | yes | no | no |
| Rating | Deferred input/display | yes | yes | no | no |
| Resizable | Deferred display | yes | yes | no | no |
| Dock | Deferred display | yes | yes | no | no |
| Chart | Deferred display | yes | yes | no | no |
| Editor | Deferred input/display | yes | yes | no | no |
| RichText | Deferred display | yes | yes | no | no |
| Carousel | Deferred display | yes | yes | no | no |
| DragDrop | Deferred input/display | yes | yes | no | no |
| Theme | Deferred display | yes | yes | no | no |
| AspectRatio | Container | yes | yes | no | no |
| Alert | Deferred feedback | yes | yes | no | no |
| Attachment | Deferred data display | yes | yes | no | no |
| Bubble | Deferred data display | yes | yes | no | no |
| Command | Deferred input | yes | yes | no | no |
| NativeSelect | Deferred input | yes | yes | no | no |
| NavigationMenu | Navigation | yes | yes | no | no |
| ScrollArea | Container | yes | yes | no | no |
| Item | Deferred data display | yes | yes | no | no |
| Message | Deferred data display | yes | yes | no | no |
| MessageScroller | Deferred data display | yes | yes | no | no |
| ToggleGroup | Form/input | yes | yes | no | no |
| Sonner | Deferred feedback | yes | yes | no | no |

## Notes

- `TimePicker` is protocol-declared and native-mapped but has no native time widget mapping yet;
  production rendering hides it and development rendering emits an explicit fallback diagnostic.
- Release certification is derived from the readiness table and fails when any approved kind is
  not semantically rendered and verified. Development fallback diagnostics never satisfy it.
- Data-display kinds express empty and populated states only. Loading/error states are not
  representable under the closed protocol schema; no renderer-side state semantics were invented.
- Overlay kinds (Dialog/AlertDialog/Popover/Sheet/BottomSheet/Drawer/Toast/Notification/
  ContextMenu/CommandPalette) are gated by host-owned dismissal state that resets whenever the
  protocol reports them closed.
